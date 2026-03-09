# Build Scripts

Platform-specific release bundle helpers.

These scripts are also exercised by the CI build matrix so alpha packaging failures show up on
PRs before a release tag is cut.

## Linux

```bash
scripts/build/build_linux_release.sh
```

Output: `dist/rust-mule-<gitsha>-linux-<arch>.tar.gz`

## macOS

```bash
scripts/build/build_macos_release.sh
```

Output: `dist/rust-mule-<gitsha>-macos-<arch>.tar.gz`

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
  - macOS
  - Windows
- `.github/workflows/release.yml` still publishes tagged release artifacts using the same
  platform-specific scripts.

The intended private alpha release flow is:

1. merge CLI/build-matrix readiness work to `main`
2. ensure the CI build matrix is green
3. tag `main` with an alpha tag such as `v0.1.0-alpha.1`
4. let the release workflow publish the packaged artifacts
