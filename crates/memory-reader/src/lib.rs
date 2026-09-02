//! Process memory reading primitives.

#![forbid(unsafe_code)]

mod error;
mod native;
mod target;

pub use error::{AccessError, DiscoveryError};
pub use native::{AccessProof, AttachedTarget, attach, discover_targets};
pub use target::{ProcessInstance, Target};
