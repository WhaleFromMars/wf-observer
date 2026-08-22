//! Staged executable validation and cross-platform replacement.

use std::fs::OpenOptions;

use anyhow::{Context as _, ensure};
use semver::Version;

use super::StagedUpdate;

pub(super) async fn probe(staged: &StagedUpdate) -> anyhow::Result<()> {
    make_executable(staged.executable())?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(staged.executable())
        .and_then(|file| file.sync_all())
        .with_context(|| format!("failed to persist {}", staged.executable().display()))?;

    let output = tokio::process::Command::new(staged.executable())
        .arg("--version")
        .output()
        .await
        .with_context(|| format!("failed to run {}", staged.executable().display()))?;
    ensure!(
        output.status.success(),
        "staged executable version probe failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );

    let stdout = std::str::from_utf8(&output.stdout)
        .context("staged executable returned a non-UTF-8 version")?;
    let reported: Version = stdout
        .split_whitespace()
        .next_back()
        .context("staged executable returned an empty version")?
        .parse()
        .context("staged executable returned invalid SemVer")?;
    ensure!(
        &reported == staged.version(),
        "staged executable reports {reported}, expected {}",
        staged.version()
    );

    Ok(())
}

pub(super) fn install(staged: &StagedUpdate) -> anyhow::Result<()> {
    self_replace::self_replace(staged.executable())
        .with_context(|| format!("failed to install version {}", staged.version()))
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to make {} executable", path.display()))
}

#[cfg(not(unix))]
fn make_executable(path: &std::path::Path) -> anyhow::Result<()> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
    ensure!(metadata.is_file(), "{} is not a file", path.display());
    Ok(())
}
