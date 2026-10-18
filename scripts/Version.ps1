# Returns release version metadata derived from Cargo.toml (the single source
# of truth). Same mapping as build.rs and scripts/version.sh:
#   Version  0.2.0-rc.1  (file names, tags, display)
#   Numeric  0.2.0.1     (Windows four-part version: rc.N -> N, release -> 1000)
$ErrorActionPreference = 'Stop'
$cargoToml = Join-Path (Split-Path $PSScriptRoot -Parent) 'Cargo.toml'
$line = Get-Content -LiteralPath $cargoToml | Where-Object { $_ -match '^version = "(.*)"$' } | Select-Object -First 1
if (-not $line) { throw 'No version found in Cargo.toml.' }
$version = ([regex]::Match($line, '^version = "(.*)"$')).Groups[1].Value
$m = [regex]::Match($version, '^(\d+)\.(\d+)\.(\d+)(-rc\.(\d+))?$')
if (-not $m.Success) { throw "Unsupported version '$version': use X.Y.Z or X.Y.Z-rc.N" }
$build = 1000
if ($m.Groups[5].Success) {
    $build = [int]$m.Groups[5].Value
    if ($build -lt 1 -or $build -gt 999) { throw 'rc number must be 1-999' }
}
[pscustomobject]@{
    Version    = $version
    Numeric    = '{0}.{1}.{2}.{3}' -f $m.Groups[1].Value, $m.Groups[2].Value, $m.Groups[3].Value, $build
    Prerelease = $m.Groups[5].Success
}
