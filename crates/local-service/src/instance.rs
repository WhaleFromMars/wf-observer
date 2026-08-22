//! Cross-process coordination for the foreground application and updater.

use std::{
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Paths used to coordinate the foreground process and updater.
#[derive(Clone, Debug)]
pub struct CoordinationPaths {
    directory: PathBuf,
}

impl CoordinationPaths {
    /// Uses `directory` for the process lock files.
    #[must_use]
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    fn instance(&self) -> PathBuf {
        self.directory.join("instance.lock")
    }

    fn update(&self) -> PathBuf {
        self.directory.join("update.lock")
    }

    fn shutdown(&self) -> PathBuf {
        self.directory.join("shutdown.lock")
    }
}

/// Exclusive lock held for the lifetime of the foreground process.
#[derive(Debug)]
pub struct InstanceGuard {
    #[allow(
        dead_code,
        reason = "holds the instance lock until the guard is dropped"
    )]
    file: File,
    paths: CoordinationPaths,
}

impl InstanceGuard {
    /// Acquires the foreground-process lock.
    ///
    /// # Errors
    ///
    /// Returns an error when the lock files cannot be accessed or another instance is running.
    pub fn acquire(paths: &CoordinationPaths) -> anyhow::Result<Self> {
        let file = try_acquire(&paths.instance())?
            .context("another local service instance is already running")?;

        Ok(Self {
            file,
            paths: paths.clone(),
        })
    }

    /// Returns whether an updater is ready for the process to shut down.
    ///
    /// # Errors
    ///
    /// Returns an error when the shutdown-request lock cannot be inspected.
    pub fn update_requested(&self) -> anyhow::Result<bool> {
        is_locked(&self.paths.shutdown())
    }

    /// Waits until an updater requests a graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error when the shutdown-request lock cannot be inspected.
    pub async fn wait_for_update_request(&self) -> anyhow::Result<()> {
        loop {
            if self.update_requested()? {
                return Ok(());
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
}

/// Exclusive lock held for the complete update operation.
#[derive(Debug)]
pub struct UpdateGuard {
    #[allow(dead_code, reason = "holds the update lock until the guard is dropped")]
    file: File,
}

impl UpdateGuard {
    /// Prevents concurrent updater processes.
    ///
    /// # Errors
    ///
    /// Returns an error when the lock cannot be accessed or an update is already running.
    pub fn acquire(paths: &CoordinationPaths) -> anyhow::Result<Self> {
        let file = try_acquire(&paths.update())?
            .context("another local service update is already in progress")?;
        Ok(Self { file })
    }
}

/// Lock that asks the foreground process to shut down for a staged update.
#[derive(Debug)]
pub struct ShutdownRequest {
    #[allow(
        dead_code,
        reason = "holds the shutdown request until the guard is dropped"
    )]
    file: File,
}

impl ShutdownRequest {
    /// Signals that a verified update is ready to install.
    ///
    /// # Errors
    ///
    /// Returns an error when the lock cannot be accessed or another request is active.
    pub fn acquire(paths: &CoordinationPaths) -> anyhow::Result<Self> {
        let file = try_acquire(&paths.shutdown())?
            .context("another updater already requested a service shutdown")?;
        Ok(Self { file })
    }
}

/// Waits for the foreground process to release its lock and holds it through replacement.
///
/// # Errors
///
/// Returns an error when the lock cannot be accessed or the timeout expires.
pub async fn wait_for_instance(
    paths: &CoordinationPaths,
    timeout: Duration,
) -> anyhow::Result<InstanceGuard> {
    tokio::time::timeout(timeout, async {
        loop {
            if let Some(file) = try_acquire(&paths.instance())? {
                return Ok(InstanceGuard {
                    file,
                    paths: paths.clone(),
                });
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    })
    .await
    .context("timed out waiting for the running local service to stop")?
}

fn open_lock(path: &Path) -> anyhow::Result<File> {
    let parent = path.parent().context("lock path does not have a parent")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))
}

fn try_acquire(path: &Path) -> anyhow::Result<Option<File>> {
    let file = open_lock(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => {
            Err(error).with_context(|| format!("failed to lock {}", path.display()))
        }
    }
}

fn is_locked(path: &Path) -> anyhow::Result<bool> {
    Ok(try_acquire(path)?.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prevents_duplicate_instances_and_updates() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let paths = CoordinationPaths::new(directory.path().to_path_buf());

        let _instance = InstanceGuard::acquire(&paths)?;
        let Err(instance_error) = InstanceGuard::acquire(&paths) else {
            anyhow::bail!("a duplicate instance unexpectedly acquired the lock");
        };
        assert!(instance_error.to_string().contains("already running"));

        let _update = UpdateGuard::acquire(&paths)?;
        let Err(update_error) = UpdateGuard::acquire(&paths) else {
            anyhow::bail!("a duplicate updater unexpectedly acquired the lock");
        };
        assert!(update_error.to_string().contains("already in progress"));

        Ok(())
    }

    #[tokio::test]
    async fn observes_only_live_shutdown_requests() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let paths = CoordinationPaths::new(directory.path().to_path_buf());
        let instance = InstanceGuard::acquire(&paths)?;

        assert!(!instance.update_requested()?);
        let request = ShutdownRequest::acquire(&paths)?;
        assert!(instance.update_requested()?);
        drop(request);
        assert!(!instance.update_requested()?);

        Ok(())
    }

    #[tokio::test]
    async fn waits_until_the_instance_exits() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let paths = CoordinationPaths::new(directory.path().to_path_buf());
        let instance = InstanceGuard::acquire(&paths)?;
        let release = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            drop(instance);
        });

        let replacement = wait_for_instance(&paths, Duration::from_secs(1)).await?;
        release.await?;
        anyhow::ensure!(
            !replacement.update_requested()?,
            "unexpected shutdown request"
        );

        Ok(())
    }
}
