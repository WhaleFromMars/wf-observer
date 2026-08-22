//! Shared local-service lifecycle and self-update support.

pub mod instance;
pub mod update;

/// Stable application identifier used for per-user storage directories.
pub const APPLICATION_ID: &str = "wf-observer";

/// Rust target triple used to select a release artifact.
pub const TARGET_TRIPLE: &str = env!("WF_OBSERVER_TARGET");
