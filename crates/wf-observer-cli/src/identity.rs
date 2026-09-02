//! Persistent Iroh identity for Warframe Observer.

use std::{
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use directories::ProjectDirs;
use iroh::SecretKey;

const IDENTITY_FILE: &str = "identity.key";

/// Loads the service identity, creating it on first use.
pub(crate) fn load_or_create() -> anyhow::Result<SecretKey> {
    load_or_create_at(&identity_path()?)
}

fn identity_path() -> anyhow::Result<PathBuf> {
    let project = ProjectDirs::from("", "", "wf-observer")
        .context("the operating system did not provide a local configuration directory")?;

    Ok(project.config_local_dir().join(IDENTITY_FILE))
}

fn load_or_create_at(path: &Path) -> anyhow::Result<SecretKey> {
    match fs::read(path) {
        Ok(bytes) => decode(&bytes, path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => create(path),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn decode(bytes: &[u8], path: &Path) -> anyhow::Result<SecretKey> {
    let bytes: &[u8; 32] = bytes.try_into().with_context(|| {
        format!(
            "{} does not contain a valid 32-byte Iroh identity",
            path.display()
        )
    })?;

    Ok(SecretKey::from_bytes(bytes))
}

fn create(path: &Path) -> anyhow::Result<SecretKey> {
    let parent = path
        .parent()
        .context("the identity path does not have a parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let secret_key = SecretKey::generate();
    let mut temporary = tempfile::Builder::new()
        .prefix(".identity.")
        .tempfile_in(parent)
        .with_context(|| {
            format!(
                "failed to create a temporary identity in {}",
                parent.display()
            )
        })?;
    temporary
        .write_all(&secret_key.to_bytes())
        .and_then(|()| temporary.as_file().sync_all())
        .with_context(|| format!("failed to persist {}", path.display()))?;

    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(secret_key),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => fs::read(path)
            .with_context(|| format!("failed to read {}", path.display()))
            .and_then(|bytes| decode(&bytes, path)),
        Err(error) => {
            Err(error.error).with_context(|| format!("failed to create {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reuses_the_identity() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join(IDENTITY_FILE);

        let first = load_or_create_at(&path)?;
        let second = load_or_create_at(&path)?;

        assert_eq!(first.to_bytes(), second.to_bytes());
        assert_eq!(fs::read(path)?, first.to_bytes());

        Ok(())
    }

    #[test]
    fn rejects_a_corrupt_identity() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join(IDENTITY_FILE);
        fs::write(&path, [0_u8; 31])?;

        let Err(error) = load_or_create_at(&path) else {
            anyhow::bail!("a corrupt identity was accepted");
        };

        assert!(error.to_string().contains("valid 32-byte Iroh identity"));
        Ok(())
    }
}
