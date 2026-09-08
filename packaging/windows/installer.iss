; Inno Setup script for the Fastcloud Windows installer.
; Build with: ISCC.exe installer.iss  (after `cargo build --release`)

#define AppName "Fastcloud"
#ifndef AppVersion
#define AppVersion "0.1.0"
#endif
#define AppPublisher "Fastcloud contributors"
#ifndef AppURL
#define AppURL "https://github.com/COMF2222/fastcloud"
#endif
#define AppExeName "fastcloud.exe"

[Setup]
AppId={{8C1F6D42-9A7B-4B8E-9E1C-FA57C10D0001}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
UninstallDisplayIcon={app}\{#AppExeName}
LicenseFile=..\..\LICENSE
OutputBaseFilename=Fastcloud-Setup-{#AppVersion}
OutputDir=..\..\dist
Compression=lzma2/max
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
WizardStyle=modern
; The app runs unelevated and stores nothing outside the user profile.
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional icons:"

[Files]
Source: "..\..\target\release\fastcloud.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\fastcloud.svg"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

; `soundcloud:` and `fastcloud:` links open in the running window; the single
; instance forwards them (see src/desktop/single_instance.rs).
[Registry]
Root: HKCU; Subkey: "Software\Classes\soundcloud"; ValueType: string; ValueName: ""; ValueData: "URL:SoundCloud Protocol"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\soundcloud"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\soundcloud\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExeName},0"
Root: HKCU; Subkey: "Software\Classes\soundcloud\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\fastcloud"; ValueType: string; ValueName: ""; ValueData: "URL:Fastcloud Protocol"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\fastcloud"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\fastcloud\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExeName},0"
Root: HKCU; Subkey: "Software\Classes\fastcloud\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent

; Settings and caches are per-user; the credentials are in Credential Manager
; and are deliberately left alone (the user may reinstall).
[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\fastcloud\cache"
