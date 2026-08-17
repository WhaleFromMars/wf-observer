//! Iroh transport endpoint and RPC handler.

use anyhow::Context as _;
use iroh::{Endpoint, endpoint::presets, protocol::Router};
use irpc::WithChannels;
use irpc_iroh::IrohProtocol;
use protocol::{ObserverMessage, ObserverProtocol, Pong};
use tokio::{sync::mpsc, task::JoinHandle};

const REQUEST_QUEUE_CAPACITY: usize = 16;

pub(crate) struct Server {
    router: Router,
    actor: JoinHandle<()>,
}

impl Server {
    pub(crate) fn endpoint(&self) -> &Endpoint {
        self.router.endpoint()
    }

    pub(crate) async fn shutdown(self) -> anyhow::Result<()> {
        let Self { router, actor } = self;
        let router_result = router
            .shutdown()
            .await
            .context("failed to shut down the Iroh router");

        drop(router);
        actor.await.context("protocol actor task failed")?;

        router_result
    }
}

pub(crate) async fn start() -> anyhow::Result<Server> {
    // TODO: Persist and reuse the secret key before enabling updates; reconnecting
    // clients depend on the service retaining its EndpointId across restarts.
    let endpoint = Endpoint::bind(presets::N0)
        .await
        .context("failed to bind the Iroh endpoint")?;

    let (requests_tx, requests_rx) = mpsc::channel(REQUEST_QUEUE_CAPACITY);
    let actor = tokio::spawn(handle_requests(requests_rx));
    let handler = IrohProtocol::<ObserverProtocol>::with_sender(requests_tx);
    let router = Router::builder(endpoint)
        .accept(protocol::ALPN_V0, handler)
        .spawn();

    Ok(Server { router, actor })
}

async fn handle_requests(mut requests: mpsc::Receiver<ObserverMessage>) {
    while let Some(request) = requests.recv().await {
        match request {
            ObserverMessage::Ping(request) => {
                let WithChannels { tx, .. } = request;
                let _ = tx.send(Pong).await;
            }
        }
    }
}
