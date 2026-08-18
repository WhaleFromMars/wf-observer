//! Async execution at the foreign-runtime boundary.

use std::future::Future;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub(crate) async fn execute<F>(future: F) -> Result<F::Output, String>
where
    F: Future,
{
    Ok(future.await)
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub(crate) async fn execute<F>(future: F) -> Result<F::Output, String>
where
    F: Future,
{
    use std::sync::OnceLock;

    use tokio::runtime::{Builder, Runtime};
    use tokio_util::context::TokioContext;

    static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();

    let runtime = RUNTIME
        .get_or_init(|| {
            Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("failed to create the WF Observer async runtime: {error}"))
        })
        .as_ref()
        .map_err(Clone::clone)?;

    Ok(TokioContext::new(future, runtime.handle().clone()).await)
}

#[cfg(all(test, not(all(target_family = "wasm", target_os = "unknown"))))]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll, Waker},
        time::Duration,
    };

    #[test]
    fn drives_tokio_futures_without_a_host_runtime() -> Result<(), String> {
        pollster::block_on(super::execute(async {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }))
    }

    #[test]
    fn cancellation_drops_the_inner_future() {
        struct PendingUntilDropped(Arc<AtomicBool>);

        impl Future for PendingUntilDropped {
            type Output = ();

            fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
                Poll::Pending
            }
        }

        impl Drop for PendingUntilDropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Relaxed);
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        let mut execution = Box::pin(super::execute(PendingUntilDropped(Arc::clone(&dropped))));
        let mut context = Context::from_waker(Waker::noop());

        assert!(execution.as_mut().poll(&mut context).is_pending());
        drop(execution);
        assert!(dropped.load(Ordering::Relaxed));
    }
}
