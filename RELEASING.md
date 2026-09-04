# Releasing WF Observer

The CLI and client SDK have independent release lifecycles. This document
covers the CLI executable. Binding artifacts continue to use the existing
Release bindings workflow.

## CLI release

1. Update `version` in `crates/wf-observer-cli/Cargo.toml` according to the
   user-facing CLI and agent changes included in the release.
2. Run the local release validation and normal repository checks:

   ```bash
   cargo run --locked -p xtask -- release check-cli --tag cli-v0.1.0
   just check
   ```

3. Merge the version change into `main`.
4. Create and push an annotated tag matching the version exactly:

   ```bash
   git tag --annotate cli-v0.1.0 --message "WF Observer CLI 0.1.0"
   git push origin cli-v0.1.0
   ```

The Release CLI workflow validates the tag, builds and smoke-tests Linux and
Windows x64 executables, packages both with the repository licences and README,
generates SHA-256 checksums, and creates the GitHub Release. Prerelease versions
such as `0.2.0-rc.1` create GitHub prereleases.

Run the workflow manually to build the complete artifact set without creating
a GitHub Release. Released tags and assets are immutable; publish a new version
instead of replacing an existing release.

## Package managers

The binary AUR and Scoop releases consume the archives and checksums produced
by the CLI workflow. The source-based AUR package downloads GitHub's archive
for the matching `cli-v{version}` tag and pins its checksum in the `PKGBUILD`.
Package publication automation is intentionally maintained separately so a
packaging failure cannot alter or replace the canonical release assets.
