# Continuous integration

This document describes the CI workflow. Where applicable, each section ends with the equivalent command for running the check locally.
I am new to CI, this probably does not follow best practices, feel free to suggest improvements.

## Core Rust Checks

The CI workflow runs formatting, Clippy, documentation, tests, and builds on
Windows and Linux for pull requests and pushes to `main`.

Rustdoc treats broken intra-doc links as errors.

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo doc --locked --workspace --no-deps --all-features
cargo test --locked --workspace --all-features
cargo build --locked --workspace --all-targets --all-features
cargo build --locked -p example-rust-dioxus --features dioxus/desktop
```

## Workflow Linting

Actionlint validates GitHub Actions workflow syntax, expressions, job
dependencies, and embedded shell scripts whenever a workflow changes.

Requires [Actionlint](https://github.com/rhysd/actionlint) 1.7.12 locally.

```bash
actionlint
```

## Language Bindings

The Bindings workflow packages C#, Java, Python, and browser bindings on Linux,
then runs the C#, Java, Kotlin/JVM, and Python console examples. A macOS job packages Swift and runs its console example.
Artifact uploads are reserved for release workflows.

`boltffi.ci.toml` limits that pull-request job to its macOS ARM64 slice; release
packaging continues to build the complete Apple matrix from `boltffi.toml`.

Generated files live under the ignored `dist/` directory.

Requires [Just](https://just.systems/), [BoltFFI](https://www.boltffi.dev/)
0.30.1, Clang, JDK 17, the .NET 10 SDK, and Python 3.10 or newer. Swift
packaging additionally requires macOS and Xcode. `pack` regenerates the binding
before building the artifact consumed by each example.

```bash
cargo install --locked --version 0.30.1 boltffi_cli
rustup target add wasm32-unknown-unknown
just binding csharp
just binding java
just binding python --python python
just binding wasm
# macOS only
just binding apple
```

The example runner packages each required binding once, starts a temporary
service, passes its endpoint ticket to every selected example, and shuts the
service down afterwards:

```bash
just example python csharp java kotlin
# macOS only
just example swift
```

The Dioxus showcase scaffold is compiled in CI. Its game-dependent behaviour
will remain local-only once implemented.

Android generation remains disabled until the generated JNI boundary installs
the JVM context required by [Iroh on Android](https://docs.rs/iroh/latest/iroh/endpoint/struct.Endpoint.html#usage-on-android).

## Release bindings

The Release bindings workflow runs for version tags and on manual request. It
builds JVM bundles and Python wheels on every native host supported by BoltFFI,
assembles one six-RID NuGet package, and packages the Apple and browser targets.
Android remains excluded until its Iroh initialization is implemented.

Python wheels cover every supported host for CPython 3.10 through 3.14. The
workflow uploads artifacts to its Actions run; publishing them to package
registries remains a separate release step.

JVM artifacts are target-specific because BoltFFI 0.30.1 cannot combine
cross-host JVM builds. The NuGet assembly job uses `boltffi.release.toml` to
combine native libraries built by the platform matrix.

The equivalent command for one local desktop target is:

```bash
just binding java --release
just binding python --release --python python
just binding csharp --release
```

Apple and browser packages can be built on their respective hosts with:

```bash
just binding apple --release
just binding wasm --release
```

## Link checking

The Links workflow uses Lychee to check links in every Markdown and Rust
source file. Rust files are scanned as plain text, which catches full URLs in doc comments and string literals.

Requires [Lychee](https://github.com/lycheeverse/lychee) to be installed
locally.

```bash
lychee './**/*.md' './**/*.rs' './**/*.toml' './**/*.yml' './**/*.yaml' './**/*.css'
```

## SemVer checking

Cargo SemVer Checks compares the public API against the latest published
version on crates.io. Its pull request trigger is disabled until a release
exists.

Requires [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks)
to be installed locally.

```bash
cargo semver-checks
```
 
## Hotpath

Hotpath is our profiling tool of choice. Profiling is not ran in CI as the workload
is dependant on a warframe.exe process running.
