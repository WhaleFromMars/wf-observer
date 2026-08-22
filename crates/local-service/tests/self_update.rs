use std::{
    collections::HashMap,
    ffi::OsStr,
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{Context as _, ensure};
use aws_lc_rs::{rand::SystemRandom, signature::Ed25519KeyPair};
use local_service::{
    instance::InstanceGuard,
    update::{UpdateConfig, UpdatePaths, execute},
};
use tempfile::TempDir;
use tough::{
    TargetName,
    editor::{RepositoryEditor, signed::PathExists},
    key_source::{KeySource, LocalKeySource},
    schema::{Role as _, RoleKeys, RoleType, Root, Signature, Signed, Target},
    sign::Sign as _,
};
use url::Url;

const CURRENT_VERSION: &str = "0.1.0";
const UPDATE_VERSION: &str = "0.1.1";

#[derive(Debug)]
struct TargetInput {
    name: TargetName,
    path: PathBuf,
}

#[derive(Debug)]
struct TestRepository {
    #[allow(
        dead_code,
        reason = "owns the temporary repository until the fixture is dropped"
    )]
    directory: TempDir,
    root: PathBuf,
    metadata: PathBuf,
    targets: PathBuf,
}

impl TestRepository {
    async fn create(
        application: &Path,
        application_target: &str,
        release_version: &str,
        metadata_version: u64,
    ) -> anyhow::Result<Self> {
        let signing = tempfile::tempdir()?;
        let root = signing.path().join("root.json");
        let key = signing.path().join("signing-key.pk8");
        create_test_root(&root, &key)?;
        Self::create_with_keys(
            application,
            application_target,
            release_version,
            metadata_version,
            &root,
            &key,
        )
        .await
    }

    async fn create_with_keys(
        application: &Path,
        application_target: &str,
        release_version: &str,
        metadata_version: u64,
        trusted_root: &Path,
        key: &Path,
    ) -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let input = directory.path().join("input");
        let metadata = directory.path().join("metadata");
        let targets = directory.path().join("targets");
        fs::create_dir_all(&input)?;
        fs::create_dir_all(&targets)?;

        let manifest = input.join("manifest.json");
        let manifest_json = serde_json::json!({
            "schema": 1,
            "version": release_version,
            "artifacts": {
                local_service::TARGET_TRIPLE: application_target,
            },
        });
        fs::write(&manifest, serde_json::to_vec(&manifest_json)?)?;

        let inputs = [
            TargetInput {
                name: TargetName::new("manifest.json")?,
                path: manifest,
            },
            TargetInput {
                name: TargetName::new(application_target)?,
                path: application.to_path_buf(),
            },
        ];

        let root = directory.path().join("root.json");
        fs::copy(trusted_root, &root)?;
        let version = NonZeroU64::new(metadata_version)
            .context("metadata version must be greater than zero")?;
        let expiration = "2099-01-01T00:00:00Z";

        let mut editor = RepositoryEditor::new(&root).await?;
        editor
            .targets_version(version)?
            .targets_expires(expiration.parse()?)?
            .snapshot_version(version)
            .snapshot_expires(expiration.parse()?)
            .timestamp_version(version)
            .timestamp_expires(expiration.parse()?);

        for target in &inputs {
            editor.add_target(target.name.clone(), Target::from_path(&target.path).await?)?;
        }

        let keys: Vec<Box<dyn KeySource>> = vec![Box::new(LocalKeySource {
            path: key.to_path_buf(),
        })];
        let signed = editor.sign(&keys).await?;
        signed.write(&metadata).await?;
        for target in &inputs {
            signed
                .copy_target(&target.path, &targets, PathExists::Fail, Some(&target.name))
                .await?;
        }

        Ok(Self {
            directory,
            root,
            metadata,
            targets,
        })
    }

    fn config(&self, state: &Path, current: &str) -> anyhow::Result<UpdateConfig> {
        let metadata_url = Url::from_directory_path(&self.metadata)
            .map_err(|()| anyhow::anyhow!("invalid metadata directory"))?;
        let targets_url = Url::from_directory_path(&self.targets)
            .map_err(|()| anyhow::anyhow!("invalid targets directory"))?;

        UpdateConfig::for_test(
            fs::read(&self.root)?,
            metadata_url,
            targets_url,
            UpdatePaths::under(state),
            current.parse()?,
            local_service::TARGET_TRIPLE.to_owned(),
            Duration::from_secs(5),
        )
    }

    fn tamper_with(&self, file_name: &str) -> anyhow::Result<()> {
        let target = find_file_named(&self.targets, file_name)
            .with_context(|| format!("could not find {file_name} in test repository"))?;
        fs::write(target, b"tampered")?;
        Ok(())
    }
}

#[tokio::test]
async fn rejects_tampered_and_rolled_back_repositories() -> anyhow::Result<()> {
    let source = tempfile::tempdir()?;
    let payload = source.path().join("payload.bin");
    fs::write(&payload, b"verified payload")?;

    let tampered = TestRepository::create(&payload, "payload.bin", UPDATE_VERSION, 1).await?;
    tampered.tamper_with("payload.bin")?;
    let state = tempfile::tempdir()?;
    assert!(
        tampered
            .config(state.path(), CURRENT_VERSION)?
            .check_for_test()
            .await
            .is_err()
    );

    let signing = tempfile::tempdir()?;
    let root = signing.path().join("root.json");
    let key = signing.path().join("signing-key.pk8");
    create_test_root(&root, &key)?;
    let newer =
        TestRepository::create_with_keys(&payload, "payload.bin", UPDATE_VERSION, 2, &root, &key)
            .await?;
    let older =
        TestRepository::create_with_keys(&payload, "payload.bin", UPDATE_VERSION, 1, &root, &key)
            .await?;
    let rollback_state = tempfile::tempdir()?;
    newer
        .config(rollback_state.path(), CURRENT_VERSION)?
        .check_for_test()
        .await?;
    assert!(
        older
            .config(rollback_state.path(), CURRENT_VERSION)?
            .check_for_test()
            .await
            .is_err()
    );

    Ok(())
}

#[tokio::test]
async fn rejects_a_version_mismatch_without_stopping_the_instance() -> anyhow::Result<()> {
    let target = PathBuf::from(env!("CARGO_BIN_EXE_update-target-fixture"));
    let repository = TestRepository::create(&target, "service.exe", "0.1.2", 1).await?;
    let state = tempfile::tempdir()?;
    let paths = UpdatePaths::under(state.path());
    let instance = InstanceGuard::acquire(paths.coordination())?;

    let Err(error) = execute(repository.config(state.path(), CURRENT_VERSION)?).await else {
        anyhow::bail!("an executable with the wrong version was unexpectedly accepted");
    };
    assert!(error.to_string().contains("reports 0.1.1, expected 0.1.2"));
    assert!(!instance.update_requested()?);

    Ok(())
}

#[tokio::test]
async fn updates_a_disposable_executable_after_the_running_instance_stops() -> anyhow::Result<()> {
    let target = PathBuf::from(env!("CARGO_BIN_EXE_update-target-fixture"));
    let target_name = format!(
        "service/{UPDATE_VERSION}/{}/wf-observer-service{}",
        local_service::TARGET_TRIPLE,
        std::env::consts::EXE_SUFFIX
    );
    let repository = TestRepository::create(&target, &target_name, UPDATE_VERSION, 1).await?;

    let installation = tempfile::tempdir()?;
    let installed = installation.path().join(format!(
        "wf-observer-service{}",
        std::env::consts::EXE_SUFFIX
    ));
    copy_executable(Path::new(env!("CARGO_BIN_EXE_update-harness")), &installed)?;

    let state = installation.path().join("runtime");
    let ready = installation.path().join("ready");
    let mut holder = tokio::process::Command::new(env!("CARGO_BIN_EXE_update-run-holder"))
        .arg(&state)
        .arg(&ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    wait_for_file(&ready).await?;

    let output = tokio::time::timeout(
        Duration::from_secs(20),
        tokio::process::Command::new(&installed)
            .arg(&repository.root)
            .arg(&repository.metadata)
            .arg(&repository.targets)
            .arg(&state)
            .output(),
    )
    .await
    .context("updater subprocess timed out")??;
    ensure!(
        output.status.success(),
        "updater failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("installed 0.1.1"));

    let holder_status = tokio::time::timeout(Duration::from_secs(5), holder.wait())
        .await
        .context("running instance did not stop")??;
    ensure!(
        holder_status.success(),
        "running instance exited unsuccessfully"
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    let version = tokio::process::Command::new(&installed)
        .arg("--version")
        .output()
        .await?;
    ensure!(version.status.success(), "installed fixture did not run");
    assert_eq!(
        String::from_utf8(version.stdout)?.trim(),
        "wf-observer-service 0.1.1"
    );

    Ok(())
}

fn create_test_root(root_path: &Path, key_path: &Path) -> anyhow::Result<()> {
    let random = SystemRandom::new();
    let key_document = Ed25519KeyPair::generate_pkcs8(&random)
        .map_err(|_| anyhow::anyhow!("failed to generate the test signing key"))?;
    fs::write(key_path, key_document.as_ref())?;
    let key_pair = Ed25519KeyPair::from_pkcs8(key_document.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to parse the generated test signing key"))?;
    let tuf_key = key_pair.tuf_key();
    let key_id = tuf_key.key_id()?;
    let role = || RoleKeys {
        keyids: vec![key_id.clone()],
        threshold: NonZeroU64::MIN,
        _extra: HashMap::new(),
    };
    let root = Root {
        spec_version: "1.0".to_owned(),
        consistent_snapshot: true,
        version: NonZeroU64::MIN,
        expires: "2099-01-01T00:00:00Z".parse()?,
        keys: HashMap::from([(key_id.clone(), tuf_key)]),
        roles: HashMap::from([
            (RoleType::Root, role()),
            (RoleType::Snapshot, role()),
            (RoleType::Targets, role()),
            (RoleType::Timestamp, role()),
        ]),
        _extra: HashMap::new(),
    };
    let signature = key_pair.sign(&root.canonical_form()?);
    let signed = Signed {
        signed: root,
        signatures: vec![Signature {
            keyid: key_id,
            sig: signature.as_ref().to_vec().into(),
        }],
    };
    fs::write(root_path, serde_json::to_vec(&signed)?)?;
    Ok(())
}

fn find_file_named(directory: &Path, file_name: &str) -> Option<PathBuf> {
    let mut pending = vec![directory.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Some(name) = path.file_name().and_then(OsStr::to_str)
                && (name == file_name || name.ends_with(&format!(".{file_name}")))
            {
                return Some(path);
            }
        }
    }
    None
}

async fn wait_for_file(path: &Path) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !path.is_file() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .with_context(|| format!("timed out waiting for {}", path.display()))
}

fn copy_executable(source: &Path, destination: &Path) -> anyhow::Result<()> {
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}
