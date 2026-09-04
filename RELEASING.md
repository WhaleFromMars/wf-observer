# Releasing WF Observer

The CLI and client SDK have independent release lifecycles. This document
covers the CLI executable. Binding artifacts continue to use the existing
Release bindings workflow.

## CLI release

1. Update `version` in `crates/wf-observer-cli/Cargo.toml` and the corresponding
   `Cargo.lock` entry, then merge the change after its pull-request CI passes.
2. From a clean, up-to-date checkout of `main`, validate and push an annotated
   tag matching the version exactly:

   ```bash
   git switch main
   git pull --ff-only origin main
   version="$(cargo run --quiet --locked -p xtask -- release check-cli)"
   tag="cli-v$version"
   cargo run --locked -p xtask -- release check-cli --tag "$tag"
   git tag --annotate "$tag" --message "WF Observer CLI $version"
   git push origin "$tag"
   ```

3. Verify that the tagged Release CLI workflow publishes the GitHub Release
   and updates `Open-WF/scoop-bucket`.

The Release CLI workflow validates the tag, builds and smoke-tests Linux and
Windows x64 executables, packages both with the repository licences and README,
generates SHA-256 checksums, and creates the GitHub Release. Prerelease versions
such as `0.2.0-rc.1` create GitHub prereleases.

Released tags and assets are immutable; publish a new version instead of
replacing an existing release.

## Packaging validation

Run the Release CLI workflow manually against `main` whenever the release
workflow, archive layout, or package templates change. The manual run builds
the complete Linux and Windows artifact set and renders package metadata
without publishing anything.

When the Windows archive or Scoop manifest changes, also install the rendered
manifest locally and exercise installation, attachment, shutdown, update, and
uninstallation. These checks are not required for an ordinary version release.

## Package managers

Stable CLI releases update `wf-observer` in `Open-WF/scoop-bucket`.
Prereleases are not sent to package managers.

The `wf-observer` and `wf-observer-bin` AUR packages are not currently
published because new account registration is unavailable and the repository
maintainers do not yet have an AUR account. Their templates and automation are
ready but disabled with a `false` guard on the `publish-aur` job in the CLI
release workflow until registration reopens.

Scoop publication requires `SCOOP_BUCKET_TOKEN`, a fine-grained token with
Contents write access to `Open-WF/scoop-bucket`.

Enabling AUR publication additionally requires `AUR_SSH_PRIVATE_KEY`, containing
a dedicated SSH key registered with the maintainers' AUR account.
