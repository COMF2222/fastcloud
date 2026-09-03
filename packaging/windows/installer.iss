; Inno Setup script for Fastcloud Windows installer
; Build with: ISCC.exe installer.iss

#define AppName "Fastcloud"
#define AppVersion "0.1.0"
#define AppPublisher "Fastcloud"
#define AppURL "https://github.com/fastcloud/fastcloud"
#define AppExeName "fastcloud.exe"

[Setup]
AppId={{8C1F6D42-9A7B-4B8E-9E1C-FASTCLOUD01}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
UninstallDisplayIcon={app}\{#AppExeName}
OutputBaseFilename=Fastcloud-Setup-{#AppVersion}
Compression=lzma2/max
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional icons:"

[Files]
Source: "..\target\release\fastcloud.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\packaging\fastcloud.svg"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent
