param(
    [Parameter(Mandatory = $true)]
    [string]$Archive
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Archive)) {
    throw "archive not found: $Archive"
}

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("rust-mule-smoke-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

try {
    Expand-Archive -Path $Archive -DestinationPath $TempDir -Force
    $ExtractDirs = @(Get-ChildItem -Path $TempDir -Directory)
    if ($ExtractDirs.Count -eq 1) {
        $ExtractRoot = $ExtractDirs[0].FullName
    }
    elseif ($ExtractDirs.Count -eq 0) {
        $ExtractRoot = $TempDir
    }
    else {
        throw "expected single extracted release directory in $Archive (found $($ExtractDirs.Count))"
    }

    $Bin = Join-Path $ExtractRoot "rust-mule.exe"
    $Cfg = Join-Path $ExtractRoot "config.example.toml"

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
