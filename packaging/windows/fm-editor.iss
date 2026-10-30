; Compile only through Tools/package-windows.py using a verified portable payload.
#ifndef PayloadDir
  #error PayloadDir is required
#endif
#ifndef OutputDir
  #error OutputDir is required
#endif
#ifndef AppVersion
  #error AppVersion is required
#endif

[Setup]
AppId={{A8BB287F-FCC2-48EA-9FD9-8E65B616AA65}
AppName=FM Editor
AppVersion={#AppVersion}
AppPublisher=FM Editor contributors
DefaultDirName={localappdata}\Programs\FM Editor
DefaultGroupName=FM Editor
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=fm-editor-{#AppVersion}-windows-x64-setup
SetupIconFile={#SourcePath}..\assets\fm-editor.ico
UninstallDisplayIcon={app}\fm-editor.ico
Uninstallable=yes
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#SourcePath}..\assets\fm-editor.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\FM Editor"; Filename: "{app}\fm-editor.exe"; IconFilename: "{app}\fm-editor.ico"
Name: "{userdesktop}\FM Editor"; Filename: "{app}\fm-editor.exe"; IconFilename: "{app}\fm-editor.ico"; Tasks: desktopicon

; No [Run], registry, or AppData cleanup: installation never launches the app,
; requests elevation, or removes settings stored outside the install directory.
