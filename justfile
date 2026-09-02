[windows]
set shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# List the available recipes.
default:
    @just --list

# Run the regular Rust checks locally.
check:
    cargo fmt --all -- --check
    cargo clippy --locked --workspace --exclude example-rust-dioxus --all-targets --all-features -- -D warnings
    cargo doc --locked --workspace --exclude example-rust-dioxus --no-deps --all-features
    cargo test --locked --workspace --exclude example-rust-dioxus --all-features
    cargo clippy --locked -p example-rust-dioxus --all-targets --features dioxus/desktop -- -D warnings
    cargo build --locked -p example-rust-dioxus --features dioxus/desktop

# Format Rust sources.
fmt *args:
    cargo fmt --all {{ args }}

# Run the Dioxus desktop showcase.
showcase *args:
    cargo run --locked -p example-rust-dioxus --features dioxus/desktop {{ args }}

# Check links in repository text and documentation files.
links *args:
    lychee {{ args }} './**/*.md' './**/*.rs' './**/*.toml' './**/*.yml' './**/*.yaml' './**/*.css'

# Run a Warframe Observer CLI command; defaults to `run`.
observer *args="run":
    cargo run --locked -p wf-observer-cli -- {{ args }}

# Package a BoltFFI target, such as `python`, `java`, `csharp`, `wasm`, or `apple`.
binding target *args:
    boltffi pack {{ target }} --deny-skipped {{ args }}

# Package bindings and run one or more console examples against a temporary service.
example +args:
    cargo run --locked -p xtask -- example {{ args }}
