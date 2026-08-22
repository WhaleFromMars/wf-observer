//! Signed release manifest parsing and platform selection.

use std::{cmp::Ordering, collections::BTreeMap};

use anyhow::{Context as _, bail, ensure};
use semver::Version;
use serde::Deserialize;
use tough::TargetName;

const SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    schema: u64,
    version: String,
    artifacts: BTreeMap<String, String>,
}

#[derive(Debug)]
pub(super) enum Selection {
    Current(Version),
    Update {
        version: Version,
        target: TargetName,
    },
}

impl Manifest {
    pub(super) fn from_slice(bytes: &[u8]) -> anyhow::Result<Self> {
        serde_json::from_slice(bytes).context("update manifest is not valid JSON")
    }

    pub(super) fn select(
        self,
        current: &Version,
        target_triple: &str,
    ) -> anyhow::Result<Selection> {
        ensure!(
            self.schema == SCHEMA_VERSION,
            "unsupported update manifest schema {}",
            self.schema
        );

        let version: Version = self
            .version
            .parse()
            .context("update manifest version is not valid SemVer")?;
        ensure!(
            version.pre.is_empty(),
            "prerelease updates are not accepted on the stable channel"
        );

        match version.cmp(current) {
            Ordering::Less => {
                bail!("refusing to downgrade from {current} to manifest version {version}")
            }
            Ordering::Equal => Ok(Selection::Current(version)),
            Ordering::Greater => {
                let target = self.artifacts.get(target_triple).with_context(|| {
                    format!("manifest does not contain an artifact for {target_triple}")
                })?;
                let target = TargetName::new(target.clone())
                    .context("manifest contains an unsafe target name")?;
                Ok(Selection::Update { version, target })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> Manifest {
        Manifest {
            schema: SCHEMA_VERSION,
            version: version.to_owned(),
            artifacts: BTreeMap::from([(
                "x86_64-test".to_owned(),
                "service/release.bin".to_owned(),
            )]),
        }
    }

    #[test]
    fn selects_only_strictly_newer_stable_versions() -> anyhow::Result<()> {
        let current = Version::new(1, 0, 0);

        assert!(matches!(
            manifest("1.0.0").select(&current, "x86_64-test")?,
            Selection::Current(version) if version == current
        ));
        assert!(matches!(
            manifest("1.1.0").select(&current, "x86_64-test")?,
            Selection::Update { version, .. } if version == Version::new(1, 1, 0)
        ));
        assert!(manifest("0.9.0").select(&current, "x86_64-test").is_err());
        assert!(
            manifest("1.1.0-rc.1")
                .select(&current, "x86_64-test")
                .is_err()
        );

        Ok(())
    }

    #[test]
    fn rejects_unknown_schemas_and_platforms() {
        let mut unknown = manifest("1.1.0");
        unknown.schema += 1;
        assert!(
            unknown
                .select(&Version::new(1, 0, 0), "x86_64-test")
                .is_err()
        );
        assert!(
            manifest("1.1.0")
                .select(&Version::new(1, 0, 0), "missing-target")
                .is_err()
        );
    }
}
