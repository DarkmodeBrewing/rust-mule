# Build Scripts

Platform-specific release bundle helpers.

These scripts are also exercised by the CI build matrix so alpha packaging failures show up on
PRs before a release tag is cut.

Artifact smoke helpers:

- `scripts/build/smoke_unix_release.sh`
- `scripts/build/smoke_windows_release.ps1`

These unpack the generated release archive and verify the packaged binary contract:

- `--version`
- `--help`
- `--check-config --config ./config.example.toml`

## Linux

```bash
scripts/build/build_linux_release.sh
```

Output: `dist/rust-mule-<gitsha>-linux-<arch>.tar.gz`

## macOS

```bash
scripts/build/build_macos_release.sh
```

Default output on Apple Silicon hosts:

- `dist/rust-mule-<gitsha>-macos-arm64.tar.gz`

Explicit target builds:

```bash
MACOS_BUILD_TARGET=aarch64-apple-darwin scripts/build/build_macos_release.sh
MACOS_BUILD_TARGET=x86_64-apple-darwin scripts/build/build_macos_release.sh
```

Outputs:

- `dist/rust-mule-<gitsha>-macos-arm64.tar.gz`
- `dist/rust-mule-<gitsha>-macos-x86_64.tar.gz`

Private alpha support floor:

- Intel macOS (`x86_64-apple-darwin`): `MACOSX_DEPLOYMENT_TARGET=12.0`
- Apple Silicon macOS (`aarch64-apple-darwin`): no explicit deployment floor is forced by default

The macOS build script applies the deployment floor only to the Intel build, because the older
private-alpha test machine is Intel macOS 12. The arm64 build remains a separate artifact.

## Windows (PowerShell)

```powershell
.\scripts\build\build_windows_release.ps1
```

Or from `cmd.exe`:

```bat
scripts\build\build_windows_release.cmd
```

Output: `dist/rust-mule-<gitsha>-windows-<arch>.zip`

## CI / Alpha Readiness

- `.github/workflows/ci.yml` now validates the host-platform packaging script on:
  - Linux
  - macOS arm64
  - macOS x86_64
  - Windows
- The CI build matrix also performs packaged-artifact smoke checks after bundling.
- `.github/workflows/release.yml` still publishes tagged release artifacts using the same
  platform-specific scripts.

The intended private alpha release flow is:

1. merge CLI/build-matrix readiness work to `main`
2. ensure the CI build matrix is green
3. tag `main` with an alpha tag such as `v0.1.0-alpha.1`
4. let the release workflow publish the packaged artifacts
