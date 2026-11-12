; Installer for Mr. PhotoPrint (Inno Setup 6.3 or newer).
; Build: iscc /DAppVersion=1.0.0 installer\MrPhotoPrint.iss   (after cargo build --release)
; Per-user install needs no administrator rights; "for all users" asks for them (UAC).
; Updates run this installer with /SILENT /CURRENTUSER|/ALLUSERS and /relaunch=1.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define AppName "Mr. PhotoPrint"
#define AppExe "MrPhotoPrint.exe"
#define AppUrl "https://github.com/MrMarkus42/mr-photoprint"

[Setup]
AppId={{57F687E9-50DE-42AB-9CF2-CE059AAEE9DB}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Mr. PhotoPrint contributors
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\dist
OutputBaseFilename=MrPhotoPrint-Setup-{#AppVersion}
SetupIconFile=..\assets\mr-photoprint.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=force
RestartApplications=no
LicenseFile=..\LICENSE-MIT
VersionInfoVersion={#AppVersion}
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\THIRD_PARTY_LICENSES.html"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent runasoriginaluser
; After an update: start the program again, never elevated (Explorer starts it as the normal user).
Filename: "{win}\explorer.exe"; Parameters: """{app}\{#AppExe}"""; Flags: nowait; Check: Relaunch

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\Mr. PhotoPrint\updates"

[Code]
function Relaunch: Boolean;
begin
  Result := ExpandConstant('{param:relaunch|0}') = '1';
end;
