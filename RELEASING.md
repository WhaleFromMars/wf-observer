# Releasing WF Observer

The CLI and client SDK have independent release lifecycles. This document
covers the CLI executable. Binding artifacts continue to use the existing
Release bindings workflow.

## CLI release

1. Update `version` in `crates/wf-observer-cli/Cargo.toml` according to the
   user-facing CLI and agent changes included in the release.
2. Run the local release validation and normal repository checks:

   ```bash
   version="$(cargo run --quiet --locked -p xtask -- release check-cli)"
   cargo run --locked -p xtask -- release check-cli --tag "cli-v$version"
   just check
   ```

3. Merge the version change into `main`.
4. Create and push an annotated tag matching the version exactly:

   ```bash
   version="$(cargo run --quiet --locked -p xtask -- release check-cli)"
   tag="cli-v$version"
   git tag --annotate "$tag" --message "WF Observer CLI $version"
   git push origin "$tag"
   ```

The Release CLI workflow validates the tag, builds and smoke-tests Linux and
Windows x64 executables, packages both with the repository licences and README,
generates SHA-256 checksums, and creates the GitHub Release. Prerelease versions
such as `0.2.0-rc.1` create GitHub prereleases.

Run the workflow manually to build the complete artifact set without creating
a GitHub Release. Released tags and assets are immutable; publish a new version
instead of replacing an existing release.

## Package managers

Stable CLI releases update `wf-observer` in `Open-WF/scoop-bucket`.
Prereleases are not sent to package managers.

The `wf-observer` and `wf-observer-bin` AUR packages are not currently
published because new account registration is unavailable and the repository
maintainers do not yet have an AUR account. Their templates and automation are
ready but disabled with a `false` guard on the `publish-aur` job in the CLI
release workflow. Until registration reopens, both packages can be built and
installed locally using the temporary flow in `packaging/README.md`.

Scoop publication requires `SCOOP_BUCKET_TOKEN`, a fine-grained token with
Contents write access to `Open-WF/scoop-bucket`.

Enabling AUR publication additionally requires `AUR_SSH_PRIVATE_KEY`, containing
a dedicated SSH key registered with the maintainers' AUR account.

A manual Release CLI workflow run for a stable version renders package metadata
without publishing it. Download the `cli-package-metadata` artifact to review
its structure. A tagged run regenerates the checksums from the published
archives before publication.
