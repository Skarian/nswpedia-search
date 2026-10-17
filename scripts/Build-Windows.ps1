param([switch]$Release)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vswhere)) { throw 'Install Visual Studio Build Tools with the Desktop development with C++ workload.' }
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'The Visual C++ workload is missing.' }
& (Join-Path $vs 'Common7\Tools\Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:PATH = "$cargoBin;$env:PATH"
Push-Location $project
try {
    $buildArgs = @('build','--locked','--bin','nswpedia-search','-j','8')
    $profile = 'debug'
    if ($Release) { $buildArgs += '--release'; $profile = 'release' }
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { throw 'Native build failed.' }
    $dest = Join-Path $project 'dist\windows'
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $project "target\$profile\nswpedia-search.exe") -Destination $dest -Force
    $runtime = Get-ChildItem (Join-Path $vs 'VC\Redist\MSVC\*\x64\Microsoft.VC143.CRT\vcruntime140.dll') |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $runtime) { throw 'Microsoft redistributable runtime was not found.' }
    Copy-Item -LiteralPath $runtime.FullName -Destination $dest -Force
    Compress-Archive -Path (Join-Path $dest '*') -DestinationPath (Join-Path $project 'dist\nswpedia-search-windows.zip') -Force
    Write-Output (Join-Path $dest 'nswpedia-search.exe')
} finally { Pop-Location }
