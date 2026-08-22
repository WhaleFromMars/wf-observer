//! Secure update discovery, staging, coordination, and replacement.

mod apply;
mod client;
mod manifest;
mod paths;

use std::time::Duration;

use anyhow::{Context as _, ensure};
use semver::Version;
use url::Url;

use client::{CheckResult, StagedUpdate, check_and_stage};
pub use paths::UpdatePaths;

use crate::instance::{ShutdownRequest, UpdateGuard, wait_for_instance};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// Immutable inputs used for one update check.
#[derive(Clone, Debug)]
pub struct UpdateConfig {
    trusted_root: Vec<u8>,
    metadata_url: Url,
    targets_url: Url,
    paths: UpdatePaths,
    current_version: Version,
    target_triple: String,
    shutdown_timeout: Duration,
}

impl UpdateConfig {
    fn from_parts(
        trusted_root: Vec<u8>,
        metadata_url: Url,
        targets_url: Url,
        paths: UpdatePaths,
        current_version: Version,
        target_triple: String,
        shutdown_timeout: Duration,
    ) -> anyhow::Result<Self> {
        ensure!(!trusted_root.is_empty(), "trusted TUF root is empty");
        ensure!(!target_triple.is_empty(), "target triple is empty");
        ensure_base_url(&metadata_url, "metadata")?;
        ensure_base_url(&targets_url, "targets")?;

        Ok(Self {
            trusted_root,
            metadata_url,
            targets_url,
            paths,
            current_version,
            target_triple,
            shutdown_timeout,
        })
    }

    /// Loads the trust root and repository URLs embedded at compile time.
    ///
    /// # Errors
    ///
    /// Returns an error when build-time configuration is absent or invalid, or paths cannot be
    /// discovered.
    pub fn production() -> anyhow::Result<Self> {
        let trusted_root = option_env!("WF_OBSERVER_TUF_ROOT_JSON")
            .context("this build does not have a trusted update repository configured")?;
        let metadata_url = option_env!("WF_OBSERVER_TUF_METADATA_URL")
            .context("this build does not have a TUF metadata URL configured")?;
        let targets_url = option_env!("WF_OBSERVER_TUF_TARGETS_URL")
            .context("this build does not have a TUF targets URL configured")?;

        let metadata_url = Url::parse(metadata_url).context("invalid embedded metadata URL")?;
        let targets_url = Url::parse(targets_url).context("invalid embedded targets URL")?;
        ensure!(
            metadata_url.scheme() == "https" && targets_url.scheme() == "https",
            "production update URLs must use HTTPS"
        );

        Self::from_parts(
            trusted_root.as_bytes().to_vec(),
            metadata_url,
            targets_url,
            UpdatePaths::discover()?,
            env!("CARGO_PKG_VERSION")
                .parse()
                .context("package version is not valid SemVer")?,
            crate::TARGET_TRIPLE.to_owned(),
            SHUTDOWN_TIMEOUT,
        )
    }

    fn trusted_root(&self) -> &[u8] {
        &self.trusted_root
    }

    fn metadata_url(&self) -> &Url {
        &self.metadata_url
    }

    fn targets_url(&self) -> &Url {
        &self.targets_url
    }

    fn paths(&self) -> &UpdatePaths {
        &self.paths
    }

    fn current_version(&self) -> &Version {
        &self.current_version
    }

    fn target_triple(&self) -> &str {
        &self.target_triple
    }
}

#[cfg(feature = "update-test-fixtures")]
impl UpdateConfig {
    /// Creates update inputs for the feature-gated disposable test harness.
    ///
    /// # Errors
    ///
    /// Returns an error when the trust root, target triple, or repository URLs are invalid.
    pub fn for_test(
        trusted_root: Vec<u8>,
        metadata_url: Url,
        targets_url: Url,
        paths: UpdatePaths,
        current_version: Version,
        target_triple: String,
        shutdown_timeout: Duration,
    ) -> anyhow::Result<Self> {
        Self::from_parts(
            trusted_root,
            metadata_url,
            targets_url,
            paths,
            current_version,
            target_triple,
            shutdown_timeout,
        )
    }

    /// Checks and stages repository contents for the feature-gated integration tests.
    ///
    /// # Errors
    ///
    /// Returns an error when metadata or targets cannot be fetched, authenticated, or persisted.
    pub async fn check_for_test(&self) -> anyhow::Result<()> {
        check_and_stage(self).await?;
        Ok(())
    }
}

/// Result of an explicit update command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateOutcome {
    /// The installed version already matches the repository manifest.
    Current(Version),
    /// A newer version replaced the updater executable.
    Installed(Version),
}

impl UpdateOutcome {
    /// Version described by the outcome.
    #[must_use]
    pub fn version(&self) -> &Version {
        match self {
            Self::Current(version) | Self::Installed(version) => version,
        }
    }
}

/// Checks, stages, and installs an update while coordinating with `run`.
///
/// # Errors
///
/// Returns an error when the update cannot be fetched, verified, probed, coordinated, or installed.
pub async fn execute(config: UpdateConfig) -> anyhow::Result<UpdateOutcome> {
    let coordination = config.paths().coordination();
    let _update = UpdateGuard::acquire(coordination)?;

    let staged = match check_and_stage(&config).await? {
        CheckResult::Current(version) => return Ok(UpdateOutcome::Current(version)),
        CheckResult::Staged(staged) => staged,
    };

    apply::probe(&staged).await?;

    let _shutdown = ShutdownRequest::acquire(coordination)?;
    let _instance = wait_for_instance(coordination, config.shutdown_timeout).await?;
    apply::install(&staged)?;

    Ok(UpdateOutcome::Installed(staged.version().clone()))
}

fn ensure_base_url(url: &Url, description: &str) -> anyhow::Result<()> {
    ensure!(
        url.path().ends_with('/'),
        "{description} base URL must end with a slash"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_directory_base_urls() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let paths = UpdatePaths::under(directory.path());
        let Err(error) = UpdateConfig::from_parts(
            vec![1],
            Url::parse("file:///metadata")?,
            Url::parse("file:///targets/")?,
            paths,
            Version::new(1, 0, 0),
            "test-target".to_owned(),
            Duration::from_secs(1),
        ) else {
            anyhow::bail!("an invalid base URL was unexpectedly accepted");
        };

        assert!(error.to_string().contains("metadata base URL"));
        Ok(())
    }
}
