use std::{path::PathBuf, process::ExitCode, time::Duration};

use anyhow::Context as _;
use local_service::update::{UpdateConfig, UpdateOutcome, UpdatePaths};
use semver::Version;
use url::Url;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let mut arguments = std::env::args_os().skip(1).map(PathBuf::from);
    let root = arguments.next().context("missing trusted-root path")?;
    let metadata = arguments.next().context("missing metadata directory")?;
    let targets = arguments.next().context("missing targets directory")?;
    let state = arguments.next().context("missing state directory")?;
    anyhow::ensure!(arguments.next().is_none(), "unexpected extra argument");

    let metadata_url = Url::from_directory_path(&metadata)
        .map_err(|()| anyhow::anyhow!("invalid metadata directory {}", metadata.display()))?;
    let targets_url = Url::from_directory_path(&targets)
        .map_err(|()| anyhow::anyhow!("invalid targets directory {}", targets.display()))?;
    let config = UpdateConfig::for_test(
        std::fs::read(&root)
            .with_context(|| format!("failed to read trusted root {}", root.display()))?,
        metadata_url,
        targets_url,
        UpdatePaths::under(&state),
        Version::new(0, 1, 0),
        local_service::TARGET_TRIPLE.to_owned(),
        Duration::from_secs(10),
    )?;

    match local_service::update::execute(config).await? {
        UpdateOutcome::Current(version) => println!("current {version}"),
        UpdateOutcome::Installed(version) => println!("installed {version}"),
    }
    Ok(())
}
