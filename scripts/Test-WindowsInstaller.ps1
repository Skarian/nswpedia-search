# Silent install / upgrade / uninstall test for the Windows installer.
#   .\scripts\Test-WindowsInstaller.ps1 -Setup dist\release\...-setup.exe [-PreviousSetup older-setup.exe] [-Launch]
# Installs into the real per-user location, checks files, shortcuts, version
# metadata and the uninstall entry, then uninstalls and checks that user data
# survived: the settings folder (a sentinel file is added beside settings.json,
# which is never modified) and the Real-Debrid credential. -SeedCredential
# writes a dummy credential under the app's keyring identity for the test and
# deletes it afterwards; it refuses to run if a real token is already stored.
param(
    [Parameter(Mandatory)][string]$Setup,
    [string]$PreviousSetup,
    [switch]$Launch,
    [switch]$SeedCredential
)
$ErrorActionPreference = 'Stop'
$meta = & (Join-Path $PSScriptRoot 'Version.ps1')
$appId = '{918D6931-5C88-4EEE-9F8B-85E0D2EA2C30}_is1'
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$appId"
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\NSWpedia Search'
$exe = Join-Path $installDir 'nswpedia-search.exe'
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\NSWpedia Search.lnk'
$desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'NSWpedia Search.lnk'
$configDir = Join-Path $env:APPDATA 'NSWpedia\NSWpedia Search\config'
$sentinel = Join-Path $configDir 'installer-test-sentinel.txt'
$credTarget = 'real-debrid-api-key.NSWpedia Search'

function Check($condition, $message) {
    if (-not $condition) { throw "FAILED: $message" }
    Write-Output "ok   $message"
}
function Run-Setup($path, [string[]]$extra) {
    $log = Join-Path $env:TEMP ("nswpedia-setup-{0}.log" -f [guid]::NewGuid())
    $argList = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=$log") + $extra
    $p = Start-Process -FilePath $path -ArgumentList $argList -Wait -PassThru
    if ($p.ExitCode -ne 0) { Get-Content $log -Tail 30; throw "$path exited with $($p.ExitCode)" }
}
# Generic credential access (the Windows Credential Manager API keyring uses).
Add-Type -Namespace NswTest -Name Cred -MemberDefinition @'
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
public struct CREDENTIAL {
    public uint Flags; public uint Type; public string TargetName; public string Comment;
    public long LastWritten; public uint CredentialBlobSize; public IntPtr CredentialBlob;
    public uint Persist; public uint AttributeCount; public IntPtr Attributes;
    public string TargetAlias; public string UserName;
}
[DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern bool CredReadW(string target, uint type, uint flags, out IntPtr cred);
[DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern bool CredWriteW(ref CREDENTIAL cred, uint flags);
[DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern bool CredDeleteW(string target, uint type, uint flags);
[DllImport("advapi32.dll")]
public static extern void CredFree(IntPtr cred);
'@
function Has-Credential {
    $ptr = [IntPtr]::Zero
    if (-not [NswTest.Cred]::CredReadW($credTarget, 1, 0, [ref]$ptr)) { return $false }
    [NswTest.Cred]::CredFree($ptr); $true
}
function Seed-Credential {
    $blob = [Text.Encoding]::Unicode.GetBytes('installer-test')
    $mem = [Runtime.InteropServices.Marshal]::AllocHGlobal($blob.Length)
    [Runtime.InteropServices.Marshal]::Copy($blob, 0, $mem, $blob.Length)
    $c = New-Object NswTest.Cred+CREDENTIAL
    $c.Type = 1; $c.TargetName = $credTarget; $c.UserName = 'real-debrid-api-key'; $c.Persist = 2
    $c.CredentialBlob = $mem; $c.CredentialBlobSize = $blob.Length
    $ok = [NswTest.Cred]::CredWriteW([ref]$c, 0)
    [Runtime.InteropServices.Marshal]::FreeHGlobal($mem)
    if (-not $ok) { throw 'Could not write test credential.' }
}

if (Test-Path $uninstallKey) { throw 'NSWpedia Search is already installed; uninstall it before running this test.' }
if (Get-Process nswpedia-search -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }) {
    throw 'An installed copy is running.'
}

New-Item -ItemType Directory -Path $configDir -Force | Out-Null
$stamp = (Get-Date).ToString('o')
Set-Content -LiteralPath $sentinel -Value $stamp -Encoding utf8
if ($SeedCredential) {
    if (Has-Credential) { throw "A Real-Debrid credential already exists; refusing to overwrite it." }
    Seed-Credential
}
$hadCredential = Has-Credential

if ($PreviousSetup) {
    Run-Setup $PreviousSetup @('/TASKS=desktopicon')
    $old = (Get-ItemProperty $uninstallKey).DisplayVersion
    Check ($old -ne $meta.Version) "previous version $old installed"
    Check (Test-Path $desktop) 'previous install created desktop shortcut'
    Run-Setup $Setup @()
    Check ((Get-ItemProperty $uninstallKey).DisplayVersion -eq $meta.Version) "upgraded $old -> $($meta.Version) in place"
    Check (Test-Path $desktop) 'desktop shortcut choice kept on upgrade'
} else {
    Run-Setup $Setup @('/TASKS=desktopicon')
    Check (Test-Path $desktop) 'desktop shortcut created'
}

$entries = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' |
    Where-Object { $_.GetValue('DisplayName') -eq 'NSWpedia Search' })
Check ($entries.Count -eq 1) 'exactly one per-user uninstall entry'
Check (-not (Test-Path "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$appId")) 'no machine-wide entry'
$entry = Get-ItemProperty $uninstallKey
Check ($entry.DisplayVersion -eq $meta.Version) "uninstall entry version $($entry.DisplayVersion)"
Check ($entry.InstallLocation.TrimEnd('\') -eq $installDir) "installed per-user to $installDir"
Check (Test-Path (Join-Path $installDir 'vcruntime140.dll')) 'runtime DLL installed'
$info = (Get-Item $exe).VersionInfo
Check ($info.ProductVersion -eq $meta.Version) "executable ProductVersion $($info.ProductVersion)"
Check ($info.FileVersionRaw.ToString() -eq $meta.Numeric) "executable FileVersion $($info.FileVersionRaw)"
Check (Test-Path $startMenu) 'Start menu shortcut'
$shell = New-Object -ComObject WScript.Shell
Check ($shell.CreateShortcut($startMenu).TargetPath -eq $exe) 'Start menu shortcut targets installed exe'

if ($Launch) {
    $p = Start-Process -FilePath $exe -PassThru
    Start-Sleep -Seconds 8
    $alive = -not $p.HasExited
    if ($alive) { Stop-Process -Id $p.Id -Force; $p.WaitForExit() }
    Check $alive 'installed app starts and stays running'
}

Run-Setup (Join-Path $installDir 'unins000.exe') @()
# The uninstaller finishes from a temporary copy; wait for it.
for ($i = 0; $i -lt 60 -and ((Test-Path $uninstallKey) -or (Test-Path $installDir)); $i++) { Start-Sleep 1 }
Check (-not (Test-Path $uninstallKey)) 'uninstall entry removed'
Check (-not (Test-Path $installDir)) 'install folder removed'
Check (-not (Test-Path $startMenu)) 'Start menu shortcut removed'
Check (-not (Test-Path $desktop)) 'desktop shortcut removed'
Check ((Get-Content -LiteralPath $sentinel -Raw).Trim() -eq $stamp) 'settings folder preserved'
if ($hadCredential) { Check (Has-Credential) 'Real-Debrid credential preserved' }
else { Write-Output 'skip credential check (no stored credential)' }

Remove-Item -LiteralPath $sentinel
if ($SeedCredential) { [NswTest.Cred]::CredDeleteW($credTarget, 1, 0) | Out-Null }
Write-Output 'Installer test passed.'
