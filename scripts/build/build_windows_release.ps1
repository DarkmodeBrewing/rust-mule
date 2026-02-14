$ErrorActionPreference = "Stop"

$Bin = "rust-mule.exe"
$TargetBin = Join-Path "target/release" $Bin

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found in PATH"
}

function Get-GitSha {
    try {
        (git rev-parse --short HEAD).Trim()
    }
    catch {
        "nogit"
    }
}

function Get-Arch {
    if ($env:PROCESSOR_ARCHITECTURE) {
        return $env:PROCESSOR_ARCHITECTURE.ToLowerInvariant()
    }
    return "unknown"
}

cargo build --release --locked --bin rust-mule

if (-not (Test-Path $TargetBin)) {
    throw "Expected $TargetBin to exist after build"
}

$OutRoot = "dist"
$OutDirName = "rust-mule-$(Get-GitSha)-windows-$(Get-Arch)"
$OutDir = Join-Path $OutRoot $OutDirName
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Copy-Item $TargetBin (Join-Path $OutDir $Bin) -Force
Copy-Item "config.toml" (Join-Path $OutDir "config.example.toml") -Force

@"
rust-mule Windows release bundle

Run:
  .\rust-mule.exe

Config:
  rust-mule reads .\config.toml from the current working directory.
  Copy config.example.toml -> config.toml and edit as needed.

Data:
  Runtime state is written under [general].data_dir (default: data/).
"@ | Set-Content -Path (Join-Path $OutDir "README.txt") -Encoding UTF8

$Zip = "$OutDir.zip"
if (Test-Path $Zip) {
    Remove-Item $Zip -Force
}
Compress-Archive -Path "$OutDir/*" -DestinationPath $Zip -Force
Write-Host "Wrote $Zip"
