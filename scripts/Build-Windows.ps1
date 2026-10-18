# Builds the Windows x64 app.
#   .\scripts\Build-Windows.ps1                 debug build in dist\windows
#   .\scripts\Build-Windows.ps1 -Release        optimized build + dist\nswpedia-search-windows.zip
#   .\scripts\Build-Windows.ps1 -Release -Installer
#       also writes versioned release assets to dist\release:
#       nswpedia-search-<version>-windows-x64-setup.exe and -portable.zip
# -Name picks the staging folder (use another name while dist\windows is running).
param([switch]$Release, [switch]$Installer, [string]$Name = 'windows')
$ErrorActionPreference = 'Stop'
if ($Installer -and -not $Release) { throw '-Installer requires -Release.' }
$project = Split-Path $PSScriptRoot -Parent
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vswhere)) { throw 'Install Visual Studio Build Tools with the Desktop development with C++ workload.' }
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'The Visual C++ workload is missing.' }
& (Join-Path $vs 'Common7\Tools\Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:PATH = "$cargoBin;$env:PATH"
$meta = & (Join-Path $PSScriptRoot 'Version.ps1')
Push-Location $project
try {
    $buildArgs = @('build','--locked','--bin','nswpedia-search','-j','8')
    $profile = 'debug'
    if ($Release) { $buildArgs += '--release'; $profile = 'release' }
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { throw 'Native build failed.' }
    $exe = Join-Path $project "target\$profile\nswpedia-search.exe"

    $info = (Get-Item -LiteralPath $exe).VersionInfo
    if ($info.ProductVersion -ne $meta.Version -or $info.FileVersion -ne $meta.Version) {
        throw "Executable version '$($info.ProductVersion)' does not match Cargo.toml '$($meta.Version)'."
    }

    $dest = Join-Path $project "dist\$Name"
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    Copy-Item -LiteralPath $exe -Destination $dest -Force

    # Bundle every Visual C++ runtime DLL the executable imports; everything
    # else it imports ships with Windows.
    $crt = Get-ChildItem (Join-Path $vs 'VC\Redist\MSVC\*\x64\Microsoft.VC14*.CRT') -Directory |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $crt) { throw 'Microsoft redistributable runtime was not found.' }
    $imports = & dumpbin /nologo /dependents $exe | ForEach-Object { $_.Trim().ToLowerInvariant() } | Where-Object { $_ -match '\.dll$' }
    $runtime = @($imports | Where-Object { $_ -match '^(vcruntime|msvcp|concrt|vccorlib)' })
    if ($runtime.Count -eq 0) { throw 'Expected the executable to import the Visual C++ runtime.' }
    foreach ($dll in $runtime) {
        $target = Join-Path $dest $dll
        Remove-Item -LiteralPath $target -Force -ErrorAction SilentlyContinue
        Copy-Item -LiteralPath (Join-Path $crt.FullName $dll) -Destination $target
    }
    Write-Output "Bundled runtime: $($runtime -join ', ') from $($crt.FullName)"
    $payload = @('nswpedia-search.exe') + $runtime

    if ($Release) {
        $zip = Join-Path $project "dist\nswpedia-search-$Name.zip"
        Compress-Archive -Path ($payload | ForEach-Object { Join-Path $dest $_ }) -DestinationPath $zip -Force
    }

    if ($Installer) {
        if ($runtime.Count -ne 1 -or $runtime[0] -ne 'vcruntime140.dll') {
            throw "Runtime imports changed ($($runtime -join ', ')); update installer\windows\nswpedia-search.iss."
        }
        $iscc = $env:ISCC
        if (-not $iscc) { $iscc = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source }
        if (-not $iscc) {
            $iscc = @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
                "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
        }
        if (-not $iscc) { throw 'Inno Setup 6 was not found. Install it or set $env:ISCC to ISCC.exe.' }
        $out = Join-Path $project 'dist\release'
        New-Item -ItemType Directory -Path $out -Force | Out-Null
        $base = "nswpedia-search-$($meta.Version)-windows-x64"
        & $iscc /Q "/DAppVersion=$($meta.Version)" "/DAppNumericVersion=$($meta.Numeric)" "/DSourceDir=$dest" `
            "/DOutputDir=$out" "/DOutputName=$base-setup" (Join-Path $project 'installer\windows\nswpedia-search.iss')
        if ($LASTEXITCODE -ne 0) { throw 'Inno Setup failed.' }
        $portable = Join-Path $out "$base-portable.zip"
        Compress-Archive -Path ($payload | ForEach-Object { Join-Path $dest $_ }) -DestinationPath $portable -Force
        Write-Output (Join-Path $out "$base-setup.exe")
        Write-Output $portable
    }
    Write-Output (Join-Path $dest 'nswpedia-search.exe')
} finally { Pop-Location }
