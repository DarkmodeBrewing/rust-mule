# Build Scripts

Platform-specific release bundle helpers.

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
