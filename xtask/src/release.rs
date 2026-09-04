use std::{path::Path, process::Command};

use anyhow::{Context as _, ensure};
use semver::Version;
use serde::Deserialize;

const CLI_PACKAGE: &str = "wf-observer-cli";
const CLI_TAG_PREFIX: &str = "cli-v";

#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
}

pub(crate) fn check_cli(root: &Path, tag: Option<&str>) -> anyhow::Result<()> {
    let version = cli_version(root)?;

    ensure!(
        version.build.is_empty(),
        "CLI releases cannot use SemVer build metadata"
    );

    if let Some(tag) = tag {
        let existing_tags = cli_release_tags(root)?;
        validate_cli_release(&version, tag, &existing_tags)?;
    }

    println!("{version}");
    Ok(())
}

fn cli_version(root: &Path) -> anyhow::Result<Version> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
        .output()
        .context("failed to query Cargo metadata")?;
    ensure!(
        output.status.success(),
        "Cargo metadata failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata = serde_json::from_slice::<CargoMetadata>(&output.stdout)
        .context("Cargo returned invalid metadata")?;
    let package = metadata
        .packages
        .into_iter()
        .find(|package| package.name == CLI_PACKAGE)
        .context("Cargo metadata did not contain the CLI package")?;

    Version::parse(&package.version).context("the CLI package version is not valid SemVer")
}

fn cli_release_tags(root: &Path) -> anyhow::Result<Vec<String>> {
    let pattern = format!("{CLI_TAG_PREFIX}*");
    let output = Command::new("git")
        .current_dir(root)
        .args(["tag", "--list", &pattern])
        .output()
        .context("failed to list existing CLI release tags")?;
    ensure!(
        output.status.success(),
        "Git failed to list existing CLI release tags: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tags = String::from_utf8(output.stdout)
        .context("Git returned a non-UTF-8 CLI release tag")?
        .lines()
        .map(str::to_owned)
        .collect();
    Ok(tags)
}

fn validate_cli_release(
    version: &Version,
    tag: &str,
    existing_tags: &[String],
) -> anyhow::Result<()> {
    let expected_tag = format!("{CLI_TAG_PREFIX}{version}");
    ensure!(
        tag == expected_tag,
        "CLI tag '{tag}' does not match package version '{expected_tag}'"
    );

    for existing_tag in existing_tags {
        if existing_tag == tag {
            continue;
        }

        let existing_version = existing_tag
            .strip_prefix(CLI_TAG_PREFIX)
            .context("an existing CLI release tag has an invalid prefix")?;
        let existing_version = Version::parse(existing_version)
            .with_context(|| format!("existing CLI release tag '{existing_tag}' is invalid"))?;
        ensure!(
            version > &existing_version,
            "CLI version {version} must be newer than existing release {existing_tag}"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_an_initial_release() -> anyhow::Result<()> {
        validate_cli_release(&Version::parse("0.1.0")?, "cli-v0.1.0", &[])
    }

    #[test]
    fn accepts_a_newer_release() -> anyhow::Result<()> {
        validate_cli_release(
            &Version::parse("0.2.0")?,
            "cli-v0.2.0",
            &["cli-v0.1.0".to_owned()],
        )
    }

    #[test]
    fn accepts_a_prerelease_after_the_previous_release() -> anyhow::Result<()> {
        validate_cli_release(
            &Version::parse("0.2.0-rc.1")?,
            "cli-v0.2.0-rc.1",
            &["cli-v0.1.0".to_owned()],
        )
    }

    #[test]
    fn rejects_a_mismatched_tag() -> anyhow::Result<()> {
        let result = validate_cli_release(&Version::parse("0.2.0")?, "cli-v0.1.0", &[]);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn rejects_a_reused_or_older_version() -> anyhow::Result<()> {
        let result = validate_cli_release(
            &Version::parse("0.1.0")?,
            "cli-v0.1.0",
            &["cli-v0.2.0".to_owned()],
        );
        assert!(result.is_err());
        Ok(())
    }
}
