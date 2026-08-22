//! TUF metadata loading and verified target staging.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, ensure};
use semver::Version;
use tempfile::TempDir;
use tough::{ExpirationEnforcement, Prefix, Repository, RepositoryLoader, TargetName};

use super::{UpdateConfig, manifest::Manifest, manifest::Selection};

const MANIFEST_TARGET: &str = "manifest.json";

/// Result of checking the authenticated release manifest.
#[derive(Debug)]
pub(super) enum CheckResult {
    /// The manifest describes the installed version.
    Current(Version),
    /// A newer target was verified and saved in a disposable staging directory.
    Staged(StagedUpdate),
}

/// Fully downloaded and TUF-verified executable target.
#[derive(Debug)]
pub(super) struct StagedUpdate {
    version: Version,
    executable: PathBuf,
    #[allow(
        dead_code,
        reason = "owns the temporary directory containing the staged executable"
    )]
    directory: TempDir,
}

impl StagedUpdate {
    /// Version reported by the authenticated manifest.
    #[must_use]
    pub(super) fn version(&self) -> &Version {
        &self.version
    }

    /// Path containing the completely verified target.
    #[must_use]
    pub(super) fn executable(&self) -> &Path {
        &self.executable
    }
}

/// Loads current metadata and stages a strictly newer platform target.
///
/// # Errors
///
/// Returns an error when metadata or targets cannot be fetched, authenticated, or persisted.
pub(super) async fn check_and_stage(config: &UpdateConfig) -> anyhow::Result<CheckResult> {
    config.paths().prepare()?;
    let repository = RepositoryLoader::new(
        &config.trusted_root(),
        config.metadata_url().clone(),
        config.targets_url().clone(),
    )
    .datastore(config.paths().datastore())
    .expiration_enforcement(ExpirationEnforcement::Safe)
    .load()
    .await
    .context("failed to load trusted update metadata")?;

    let directory = tempfile::Builder::new()
        .prefix("update-")
        .tempdir_in(config.paths().staging())
        .context("failed to create the update staging directory")?;
    let manifest_name = TargetName::new(MANIFEST_TARGET)?;
    save_target(&repository, &manifest_name, directory.path()).await?;
    let manifest_path = directory.path().join(MANIFEST_TARGET);
    let manifest = tokio::fs::read(&manifest_path)
        .await
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;

    match Manifest::from_slice(&manifest)?
        .select(config.current_version(), config.target_triple())?
    {
        Selection::Current(version) => Ok(CheckResult::Current(version)),
        Selection::Update { version, target } => {
            save_target(&repository, &target, directory.path()).await?;
            let executable = directory.path().join(target.resolved());
            ensure!(
                executable.is_file(),
                "verified target was not saved to {}",
                executable.display()
            );
            Ok(CheckResult::Staged(StagedUpdate {
                version,
                executable,
                directory,
            }))
        }
    }
}

async fn save_target(
    repository: &Repository,
    target: &TargetName,
    directory: &Path,
) -> anyhow::Result<()> {
    repository
        .save_target(target, directory, Prefix::None)
        .await
        .with_context(|| format!("failed to download verified target {}", target.raw()))?;
    Ok(())
}
