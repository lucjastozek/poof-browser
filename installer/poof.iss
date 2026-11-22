; Inno Setup script. Built by .github/workflows/release.yml, which passes:
;   /DAppVersion=1.2.3   /DExeName=poof   (the executable name without .exe)
#ifndef AppVersion
  #error Pass /DAppVersion=x.y.z
#endif
#ifndef ExeName
  #error Pass /DExeName=<binary name>
#endif

[Setup]
; Never change AppId: it is how updates find the existing installation.
AppId={{49447D14-E729-419C-B440-FAF11F1AC32C}
AppName=Poof
AppVersion={#AppVersion}
AppPublisher=Poof
; Per-user install: no administrator prompt, and silent updates work without elevation.
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\Poof
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename={#ExeName}-{#AppVersion}-setup
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#ExeName}.exe
; Silent updates replace a running copy cleanly and never relaunch it.
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "..\target\release\{#ExeName}.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Poof"; Filename: "{app}\{#ExeName}.exe"
Name: "{autodesktop}\Poof"; Filename: "{app}\{#ExeName}.exe"; Tasks: desktopicon

[Run]
; Not run for silent installs, so an update applied when the browser closes does not reopen it.
Filename: "{app}\{#ExeName}.exe"; Description: "Launch Poof"; Flags: nowait postinstall skipifsilent

[Code]
// The browser starts this installer while it is still shutting down; give it a moment to exit.
function InitializeSetup(): Boolean;
begin
  if WizardSilent then
    Sleep(3000);
  Result := True;
end;
