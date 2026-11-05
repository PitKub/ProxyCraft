; ProxyCraft installer (Inno Setup 6). build.bat compiles it after building dist\.
;   ISCC.exe /DAppVersion=1.0.1 installer\ProxyCraft.iss
;
; The installer asks whether to install for the current user (no administrator
; rights, default folder %LOCALAPPDATA%\Programs\ProxyCraft) or for all users
; (C:\Program Files\ProxyCraft), and lets the user choose another folder.

#ifndef AppVersion
  #define AppVersion "1.0.1"
#endif

[Setup]
; Never change AppId: Windows uses it to recognise ProxyCraft for upgrades and uninstalling.
AppId={{1BDF2899-8598-420E-8C23-CE263FD20CBF}
AppName=ProxyCraft
AppVersion={#AppVersion}
AppVerName=ProxyCraft {#AppVersion}
AppPublisher=Piotr Kubicki
AppPublisherURL=https://github.com/PitKub/ProxyCraft
AppSupportURL=https://github.com/PitKub/ProxyCraft/issues
AppUpdatesURL=https://github.com/PitKub/ProxyCraft/releases
AppCopyright=Copyright (c) 2026 Piotr Kubicki. MIT License.
VersionInfoVersion={#AppVersion}.0
DefaultDirName={autopf}\ProxyCraft
DefaultGroupName=ProxyCraft
DisableProgramGroupPage=yes
; Always show the folder page, so the user sees the default and can change it.
DisableDirPage=no
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=ProxyCraft-{#AppVersion}-Setup
SetupIconFile=..\assets\ProxyCraft.ico
UninstallDisplayIcon={app}\ProxyCraft.exe
UninstallDisplayName=ProxyCraft {#AppVersion}
WizardStyle=modern
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "polish"; MessagesFile: "compiler:Languages\Polish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\dist\ProxyCraft.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\ffmpeg\*"; DestDir: "{app}\ffmpeg"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\dist\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\ProxyCraft"; Filename: "{app}\ProxyCraft.exe"
Name: "{autodesktop}\ProxyCraft"; Filename: "{app}\ProxyCraft.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ProxyCraft.exe"; Description: "{cm:LaunchProgram,ProxyCraft}"; Flags: nowait postinstall skipifsilent
