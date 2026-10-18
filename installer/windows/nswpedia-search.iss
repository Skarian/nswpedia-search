; Inno Setup 6 script for the per-user Windows x64 installer.
; Built by scripts/Build-Windows.ps1 -Installer, which passes:
;   /DAppVersion=0.2.0-rc.1 /DAppNumericVersion=0.2.0.1
;   /DSourceDir=<folder with nswpedia-search.exe and vcruntime140.dll>
;   /DOutputDir=<folder> /DOutputName=<file name without .exe>
;
; Only files under {app} are installed and removed. Settings
; (%APPDATA%\NSWpedia\NSWpedia Search\config), the cache, the Real-Debrid
; token in Windows Credential Manager and downloaded files are never touched,
; so they survive upgrades and uninstall.

#ifndef AppVersion
  #error Pass /DAppVersion (see scripts/Build-Windows.ps1)
#endif

#define AppName "NSWpedia Search"
#define AppExe "nswpedia-search.exe"

[Setup]
; Stable upgrade identity. Never change this GUID.
AppId={{918D6931-5C88-4EEE-9F8B-85E0D2EA2C30}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Skarian
AppCopyright=MIT License
VersionInfoVersion={#AppNumericVersion}
VersionInfoProductVersion={#AppNumericVersion}
VersionInfoTextVersion={#AppVersion}
VersionInfoProductTextVersion={#AppVersion}
VersionInfoDescription={#AppName} Setup
; Per-user install into %LOCALAPPDATA%\Programs without administrator rights.
PrivilegesRequired=lowest
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
SetupIconFile=..\..\assets\icon\nswpedia-search.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
WizardStyle=modern dynamic
Compression=lzma2/max
SolidCompression=yes
; Ask to close a running copy so its files can be replaced during upgrades.
CloseApplications=yes
RestartApplications=no
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
; Microsoft Visual C++ runtime, deployed app-locally (no admin, no system install).
Source: "{#SourceDir}\vcruntime140.dll"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
