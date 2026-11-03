; FreeWriter-Setup.exe: an ordinary setup wizard for Windows, built with
; Inno Setup 6 from the release build:
;
;     cargo build --release
;     ISCC.exe /DAppVersion=1.0.0 installer\windows\freewriter.iss
;
; It installs for the current user only, so no administrator password is
; needed, in the same folder install.ps1 uses.

#ifndef AppVersion
  #error Pass the version: ISCC.exe /DAppVersion=1.0.0
#endif
#define Root "..\.."

[Setup]
; Updates find the installed FreeWriter by this id: never change it.
AppId={{CCBDDFE6-4128-4C4C-973C-181E731FF6F6}
AppName=FreeWriter
AppVersion={#AppVersion}
AppVerName=FreeWriter {#AppVersion}
AppPublisherURL=https://github.com/ReallyTheRealCodeGod/FreeWriter
AppSupportURL=https://github.com/ReallyTheRealCodeGod/FreeWriter/issues
VersionInfoVersion={#AppVersion}
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\FreeWriter
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
UninstallDisplayName=FreeWriter
UninstallDisplayIcon={app}\freewriter.exe
SetupIconFile={#Root}\assets\freewriter.ico
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#Root}\target\installer
OutputBaseFilename=FreeWriter-Setup
Compression=lzma2
SolidCompression=yes

[Tasks]
Name: desktopicon; Description: "Put FreeWriter on the desktop too"; Flags: unchecked

[Files]
Source: "{#Root}\target\release\freewriter.exe"; DestDir: "{app}"; Flags: ignoreversion

[InstallDelete]
; Left by install.ps1, which this replaces.
Type: files; Name: "{app}\uninstall.ps1"

[Icons]
Name: "{autoprograms}\FreeWriter"; Filename: "{app}\freewriter.exe"; Comment: "A distraction-free writing app that works like an old typewriter"
Name: "{autodesktop}\FreeWriter"; Filename: "{app}\freewriter.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\freewriter.exe"; Description: "Start FreeWriter"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
begin
  // install.ps1 makes its own entry in Settings > Apps; this one takes over.
  if CurStep = ssInstall then
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeWriter');
end;
