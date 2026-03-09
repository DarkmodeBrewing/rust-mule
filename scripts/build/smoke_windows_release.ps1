$ErrorActionPreference = "Stop"

param(
    [Parameter(Mandatory = $true)]
    [string]$Archive
)

if (-not (Test-Path $Archive)) {
    throw "archive not found: $Archive"
}

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("rust-mule-smoke-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

try {
    Expand-Archive -Path $Archive -DestinationPath $TempDir -Force
    $ExtractRoot = Get-ChildItem -Path $TempDir -Directory | Select-Object -First 1
    if (-not $ExtractRoot) {
        throw "expected extracted release directory in $Archive"
    }

    $Bin = Join-Path $ExtractRoot.FullName "rust-mule.exe"
    $Cfg = Join-Path $ExtractRoot.FullName "config.example.toml"

    if (-not (Test-Path $Bin)) {
        throw "expected executable $Bin"
    }

    if (-not (Test-Path $Cfg)) {
        throw "expected config example $Cfg"
    }

    & $Bin --version | Out-Null
    & $Bin --help | Out-Null
    & $Bin --check-config --config $Cfg | Out-Null

    Write-Host "smoke OK: $Archive"
}
finally {
    if (Test-Path $TempDir) {
        Remove-Item $TempDir -Recurse -Force
    }
}
