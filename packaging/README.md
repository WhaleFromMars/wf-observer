# CLI packages

Templates for publishing the WF Observer CLI to the AUR and Scoop.

`render.sh` replaces these values in the templates:

- `@VERSION@` with the CLI version;
- `@SOURCE_SHA256@` with the tagged source archive checksum;
- `@LINUX_SHA256@` with the Linux release archive checksum;
- `@WINDOWS_SHA256@` with the Windows release archive checksum.

```bash
bash packaging/render.sh \
  VERSION SOURCE_SHA256 LINUX_SHA256 WINDOWS_SHA256 OUTPUT
```

## AUR

The AUR packages are not currently published because new account registration
is unavailable and the repository maintainers do not yet have an AUR account.
Their templates and release automation remain ready for when registration
reopens.

- `aur/wf-observer/PKGBUILD.in`: builds from source.
- `aur/wf-observer-bin/PKGBUILD.in`: installs the Linux release archive.

Until AUR publication is available, render the templates locally, place the
matching source or release archive beside its rendered `PKGBUILD`, and run the
normal package commands below. `makepkg` will use the local archive, so no AUR
account or published package is required for testing.

Validate each rendered `PKGBUILD`:

```bash
updpkgsums
makepkg --printsrcinfo > .SRCINFO
namcap PKGBUILD
makepkg --cleanbuild --syncdeps
namcap ./*.pkg.tar.zst
```

Install the resulting package and exercise the public commands before
publication:

```bash
sudo pacman -U ./*.pkg.tar.zst
wf-observer --version
wf-observer --help
wf-observer status
sudo pacman -Rns wf-observer      # source-built package
sudo pacman -Rns wf-observer-bin  # binary package
```

## Scoop

Before publishing a release, build and install the Windows archive locally:

```powershell
$metadata = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
$version = ($metadata.packages | Where-Object name -eq 'wf-observer-cli').version
$target = 'x86_64-pc-windows-msvc'
$testRoot = Join-Path $env:TEMP "wf-observer-scoop-$([guid]::NewGuid())"
$staging = Join-Path $testRoot 'staging'
$archive = Join-Path $testRoot "wf-observer-$version-$target.zip"
$manifestPath = Join-Path $testRoot 'wf-observer.json'

$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --locked --release --package wf-observer-cli --target $target

New-Item -ItemType Directory -Force $staging | Out-Null
Copy-Item "target/$target/release/wf-observer.exe" $staging
Copy-Item README.md, LICENSE-MIT, LICENSE-APACHE $staging
Compress-Archive -Path "$staging/*" -DestinationPath $archive

$hash = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
$manifestText = Get-Content packaging/scoop/wf-observer.json.in -Raw
$manifestText = $manifestText.Replace('@VERSION@', $version)
$manifestText = $manifestText.Replace('@WINDOWS_SHA256@', $hash)
$manifest = $manifestText | ConvertFrom-Json
$manifest.architecture.'64bit'.url = $archive
$manifest | ConvertTo-Json -Depth 10 | Set-Content -Encoding utf8 $manifestPath

scoop install $manifestPath
scoop which wf-observer
wf-observer --version
wf-observer --help
wf-observer status
scoop uninstall wf-observer
```

Render `scoop/wf-observer.json.in` as
`Open-WF/scoop-bucket/bucket/wf-observer.json`, then validate it:

```powershell
.\bin\checkver.ps1 wf-observer .\bucket
.\bin\checkver.ps1 wf-observer .\bucket -ForceUpdate -Update
scoop install .\bucket\wf-observer.json
wf-observer --version
wf-observer --help
wf-observer status
scoop uninstall wf-observer
```

Stop an attached agent before updating:

```powershell
wf-observer stop
scoop update wf-observer
wf-observer attach
```
