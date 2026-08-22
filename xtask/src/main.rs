use std::{
    io::{BufRead as _, BufReader},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command as ProcessCommand, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context as _, bail, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use fastant::Instant;
use tempfile::TempDir;

const TICKET_PREFIX: &str = "WF_OBSERVER_ENDPOINT_TICKET=";
const START_TIMEOUT: Duration = Duration::from_secs(45);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Packages bindings and runs their console examples against a temporary service.
    Example {
        /// Languages whose console examples should run.
        #[arg(required = true, value_enum)]
        languages: Vec<Language>,
        /// Uses bindings already present under `dist` instead of packaging them.
        #[arg(long)]
        no_package: bool,
        /// Python interpreter used to build and test the Python wheel.
        #[arg(long, default_value = "python")]
        python: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Language {
    Python,
    Csharp,
    Java,
    Kotlin,
    Swift,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindingTarget {
    Python,
    Csharp,
    Java,
    Apple,
}

impl Language {
    const fn binding_target(self) -> BindingTarget {
        match self {
            Self::Python => BindingTarget::Python,
            Self::Csharp => BindingTarget::Csharp,
            Self::Java | Self::Kotlin => BindingTarget::Java,
            Self::Swift => BindingTarget::Apple,
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::Csharp => "C#",
            Self::Java => "Java",
            Self::Kotlin => "Kotlin/JVM",
            Self::Swift => "Swift",
        }
    }
}

impl BindingTarget {
    const fn boltffi_name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Csharp => "csharp",
            Self::Java => "java",
            Self::Apple => "apple",
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Example {
            languages,
            no_package,
            python,
        } => run_examples(&languages, no_package, &python),
    }
}

fn run_examples(languages: &[Language], no_package: bool, python: &Path) -> anyhow::Result<()> {
    if languages.contains(&Language::Swift) && !cfg!(target_os = "macos") {
        bail!("the Swift console example requires macOS");
    }

    let root = workspace_root()?;

    if !no_package {
        package_bindings(root, languages, python)?;
    }

    let python_environment = languages
        .contains(&Language::Python)
        .then(|| prepare_python_environment(root, python))
        .transpose()?;
    let service_binary = build_service(root)?;
    let (mut service, ticket) = RunningService::start(root, &service_binary)?;

    let examples_result = languages.iter().try_for_each(|&language| {
        run_example(root, language, &ticket, python_environment.as_ref())
    });
    let shutdown_result = service.shutdown();

    examples_result?;
    shutdown_result
}

fn workspace_root() -> anyhow::Result<&'static Path> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask must be located directly beneath the workspace root")
}

fn package_bindings(root: &Path, languages: &[Language], python: &Path) -> anyhow::Result<()> {
    let mut targets = Vec::new();

    for &language in languages {
        let target = language.binding_target();
        if !targets.contains(&target) {
            targets.push(target);
        }
    }

    for target in targets {
        let target_name = target.boltffi_name();
        let mut command = ProcessCommand::new("boltffi");
        command
            .current_dir(root)
            .args(["pack", target_name, "--deny-skipped"]);

        if target == BindingTarget::Python {
            command.arg("--python").arg(python);
        }

        run_command(&mut command, &format!("package the {target_name} binding"))?;
    }

    Ok(())
}

struct PythonEnvironment {
    _directory: TempDir,
    executable: PathBuf,
}

fn prepare_python_environment(root: &Path, python: &Path) -> anyhow::Result<PythonEnvironment> {
    let wheelhouse = root.join("dist/python/wheelhouse");
    ensure!(
        wheelhouse.is_dir(),
        "Python wheelhouse does not exist at {}; package the Python binding first",
        wheelhouse.display()
    );

    let directory =
        tempfile::tempdir().context("failed to create a temporary Python environment")?;
    let mut create = ProcessCommand::new(python);
    create
        .current_dir(root)
        .args(["-m", "venv"])
        .arg(directory.path());
    run_command(&mut create, "create the temporary Python environment")?;

    let executable = if cfg!(windows) {
        directory.path().join("Scripts/python.exe")
    } else {
        directory.path().join("bin/python")
    };
    ensure!(
        executable.is_file(),
        "temporary Python interpreter was not created at {}",
        executable.display()
    );

    let mut install = ProcessCommand::new(&executable);
    install
        .current_dir(root)
        .args([
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "--no-index",
            "--find-links",
        ])
        .arg(&wheelhouse)
        .arg(format!("wf-observer=={}", env!("CARGO_PKG_VERSION")));
    run_command(&mut install, "install the generated Python wheel")?;

    Ok(PythonEnvironment {
        _directory: directory,
        executable,
    })
}

fn build_service(root: &Path) -> anyhow::Result<PathBuf> {
    let mut build = ProcessCommand::new("cargo");
    build
        .current_dir(root)
        .args(["build", "--locked", "--quiet", "-p", "local-service"]);
    run_command(&mut build, "build the local service")?;

    let metadata = ProcessCommand::new("cargo")
        .current_dir(root)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("failed to query Cargo metadata")?;
    ensure!(
        metadata.status.success(),
        "Cargo metadata failed with {}: {}",
        metadata.status,
        String::from_utf8_lossy(&metadata.stderr)
    );

    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).context("Cargo returned invalid metadata")?;
    let target_directory = metadata
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .context("Cargo metadata did not include its target directory")?;
    let service_binary = Path::new(target_directory).join("debug").join(format!(
        "wf-observer-service{}",
        std::env::consts::EXE_SUFFIX
    ));

    ensure!(
        service_binary.is_file(),
        "local service binary was not created at {}",
        service_binary.display()
    );

    Ok(service_binary)
}

fn run_example(
    root: &Path,
    language: Language,
    ticket: &str,
    python: Option<&PythonEnvironment>,
) -> anyhow::Result<()> {
    let action = format!("run the {} console example", language.display_name());

    match language {
        Language::Python => {
            let python = python.context("the Python environment was not prepared")?;
            let mut command = ProcessCommand::new(&python.executable);
            command
                .current_dir(root)
                .arg(root.join("examples/python/console/main.py"))
                .arg(ticket);
            run_command(&mut command, &action)
        }
        Language::Csharp => {
            let packages =
                tempfile::tempdir().context("failed to create a temporary NuGet cache")?;
            let mut command = ProcessCommand::new("dotnet");
            command
                .current_dir(root)
                .env("NUGET_PACKAGES", packages.path())
                .args(["run", "--project"])
                .arg(root.join("examples/csharp/console"))
                .arg("--")
                .arg(ticket);
            run_command(&mut command, &action)
        }
        Language::Java | Language::Kotlin => {
            let task = match language {
                Language::Java => ":java:console:run",
                Language::Kotlin => ":kotlin:console:run",
                _ => unreachable!(),
            };
            let mut command = gradle_command(root);
            command
                .current_dir(root)
                .args(["-p", "examples", "--no-daemon", task])
                .arg(format!("--args={ticket}"));
            run_command(&mut command, &action)
        }
        Language::Swift => {
            let mut command = ProcessCommand::new("swift");
            command
                .current_dir(root)
                .args(["run", "--package-path"])
                .arg(root.join("examples/swift/console"))
                .arg("WFObserverConsole")
                .arg(ticket);
            run_command(&mut command, &action)
        }
    }
}

fn gradle_command(root: &Path) -> ProcessCommand {
    if cfg!(windows) {
        ProcessCommand::new(root.join("examples/gradlew.bat"))
    } else {
        let mut command = ProcessCommand::new("bash");
        command.arg(root.join("examples/gradlew"));
        command
    }
}

fn run_command(command: &mut ProcessCommand, action: &str) -> anyhow::Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to {action}"))?;
    ensure!(status.success(), "failed to {action}: {status}");
    Ok(())
}

type ServiceOutput = Result<String, String>;

struct RunningService {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    reader: Option<JoinHandle<()>>,
}

impl RunningService {
    fn start(root: &Path, binary: &Path) -> anyhow::Result<(Self, String)> {
        let child = ProcessCommand::new(binary)
            .current_dir(root)
            .args(["run", "--print-ticket", "--shutdown-on-stdin-close"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("failed to start the local service")?;
        let mut service = Self {
            child: Some(child),
            stdin: None,
            reader: None,
        };
        let child = service
            .child
            .as_mut()
            .context("local service process was not retained")?;
        service.stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .context("local service stdout was not captured")?;
        let (sender, receiver) = mpsc::channel();
        service.reader = Some(thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if sender.send(Ok(line)).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        drop(sender.send(Err(error.to_string())));
                        return;
                    }
                }
            }
        }));

        let ticket = service.wait_for_ticket(&receiver)?;
        Ok((service, ticket))
    }

    fn wait_for_ticket(&mut self, receiver: &Receiver<ServiceOutput>) -> anyhow::Result<String> {
        let deadline = Instant::now() + START_TIMEOUT;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!("timed out waiting for the local service endpoint ticket");
            }

            match receiver.recv_timeout(remaining) {
                Ok(Ok(line)) => {
                    if let Some(ticket) = line.strip_prefix(TICKET_PREFIX) {
                        ensure!(!ticket.is_empty(), "local service printed an empty ticket");
                        return Ok(ticket.to_owned());
                    }
                    println!("{line}");
                }
                Ok(Err(error)) => bail!("failed to read local service output: {error}"),
                Err(RecvTimeoutError::Timeout) => {
                    bail!("timed out waiting for the local service endpoint ticket");
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let status = self
                        .child
                        .as_mut()
                        .context("local service process was not retained")?
                        .try_wait()
                        .context("failed to inspect the local service")?;

                    if let Some(status) = status {
                        bail!("local service exited before printing a ticket: {status}");
                    }
                    bail!("local service output closed before a ticket was printed");
                }
            }
        }
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.stdin.take();
        let status = wait_for_exit(
            self.child
                .as_mut()
                .context("local service process was not retained")?,
            SHUTDOWN_TIMEOUT,
        )?;

        let Some(status) = status else {
            let child = self
                .child
                .as_mut()
                .context("local service process was not retained")?;
            child.kill().context("failed to stop the local service")?;
            child
                .wait()
                .context("failed to reap the local service process")?;
            self.child.take();
            self.join_reader()?;
            bail!("local service did not shut down within {SHUTDOWN_TIMEOUT:?}");
        };

        self.child.take();
        self.join_reader()?;
        ensure!(
            status.success(),
            "local service exited unsuccessfully: {status}"
        );
        Ok(())
    }

    fn join_reader(&mut self) -> anyhow::Result<()> {
        if let Some(reader) = self.reader.take() {
            reader
                .join()
                .map_err(|_| anyhow::anyhow!("local service output reader panicked"))?;
        }
        Ok(())
    }
}

impl Drop for RunningService {
    fn drop(&mut self) {
        self.stdin.take();

        if let Some(mut child) = self.child.take()
            && !matches!(
                wait_for_exit(&mut child, Duration::from_secs(1)),
                Ok(Some(_))
            )
        {
            drop(child.kill());
            drop(child.wait());
        }

        if let Some(reader) = self.reader.take() {
            drop(reader.join());
        }
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    }
}
