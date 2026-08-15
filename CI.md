# Continuous integration

This document describes the CI workflow. Where applicable, each section ends with the equivalent command for running the check locally.

## Core Rust Checks

The CI workflow runs formatting, Clippy, documentation, tests, and builds on
Windows and Linux for pull requests and pushes to `main`.

Rustdoc treats broken intra-doc links as errors.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps --all-features
cargo test --workspace --all-features
cargo build --workspace --all-targets --all-features
cargo build -p showcase-dioxus --features dioxus/desktop
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
