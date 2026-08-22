//! Persistent metadata, staging, and coordination paths.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use directories::ProjectDirs;

use crate::instance::CoordinationPaths;

/// Filesystem locations used by the updater.
#[derive(Clone, Debug)]
pub struct UpdatePaths {
    datastore: PathBuf,
    staging: PathBuf,
    coordination: CoordinationPaths,
}

impl UpdatePaths {
    /// Resolves the current user's update directories.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system does not expose per-user directories.
    pub fn discover() -> anyhow::Result<Self> {
        let project = ProjectDirs::from("", "", crate::APPLICATION_ID)
            .context("the operating system did not provide local application directories")?;
        let state = project.data_local_dir().join("update");
        let cache = project.cache_dir().join("update");

        Ok(Self::new(&state, &cache))
    }

    /// Places every update file below `root`, primarily for disposable tests.
    #[must_use]
    pub fn under(root: &Path) -> Self {
        Self::new(&root.join("state"), &root.join("cache"))
    }

    fn new(state: &Path, cache: &Path) -> Self {
        Self {
            datastore: state.join("metadata"),
            staging: cache.join("staging"),
            coordination: CoordinationPaths::new(state.join("locks")),
        }
    }

    pub(super) fn prepare(&self) -> anyhow::Result<()> {
        for directory in [&self.datastore, &self.staging] {
            std::fs::create_dir_all(directory)
                .with_context(|| format!("failed to create {}", directory.display()))?;
        }
        Ok(())
    }

    pub(super) fn datastore(&self) -> &Path {
        &self.datastore
    }

    pub(super) fn staging(&self) -> &Path {
        &self.staging
    }

    /// Cross-process lock paths shared by `run` and `update`.
    #[must_use]
    pub fn coordination(&self) -> &CoordinationPaths {
        &self.coordination
    }
}
