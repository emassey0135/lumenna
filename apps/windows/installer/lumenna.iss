; Lumenna's Windows installer: the app and `lum` together, for one person, with no administrator.
;
; Built by the release workflow (`.github/workflows/release.yml`), once per architecture:
;
;   iscc /DAppVersion=0.1.0 /DArch=x64 /DSource=<folder with lumenna.exe and lum.exe>
;        /DOutput=<folder> apps\windows\installer\lumenna.iss
;
; The two share one store as they always do, the profile's own (%LOCALAPPDATA%\lumenna), and `lum`
; is put on the person's PATH. Unsigned, as winget accepts and a direct download allows.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef Source
  #define Source "..\..\..\target\release"
#endif
#ifndef Output
  #define Output "..\..\..\target\installer"
#endif

; The notification identity the app gives itself (toast.rs), which the Start menu shortcut carries
; so Windows ties the app's notifications to it.
#define AppUserModelId "dev.emassey0135.Lumenna"

[Setup]
; Never to change: Windows knows an upgrade, and winget an installed copy, by it.
AppId={{4FFFD1BB-2C81-432A-9E26-1F7935D5B7D0}
AppName=Lumenna
AppVersion={#AppVersion}
AppVerName=Lumenna {#AppVersion}
AppPublisher=Elijah Massey
AppPublisherURL=https://github.com/emassey0135/lumenna
AppSupportURL=https://github.com/emassey0135/lumenna/issues
VersionInfoVersion={#AppVersion}
; For the person installing it alone: into %LOCALAPPDATA%\Programs, their PATH and their Start menu.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Lumenna
DisableProgramGroupPage=yes
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0.17763
ChangesEnvironment=yes
; The app stays running in the notification area: closed before files are replaced, not restarted.
CloseApplications=yes
RestartApplications=no
SetupIconFile=..\lumenna.ico
UninstallDisplayIcon={app}\lumenna.exe
UninstallDisplayName=Lumenna
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir={#Output}
OutputBaseFilename=Lumenna-{#AppVersion}-{#Arch}-setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Source}\lumenna.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\lum.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"

[Icons]
Name: "{autoprograms}\Lumenna"; Filename: "{app}\lumenna.exe"; AppUserModelID: "{#AppUserModelId}"
Name: "{autodesktop}\Lumenna"; Filename: "{app}\lumenna.exe"; AppUserModelID: "{#AppUserModelId}"; Tasks: desktopicon

[Registry]
; `lum` on the person's PATH; taken off again on uninstalling (RemoveFromPath, below).
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))
; What the app writes itself, taken away with it: opening at sign-in (settings.rs's Run value) and
; its notification identity (toast.rs). Written by the app, so created by nothing here.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Lumenna"; Flags: uninsdeletevalue dontcreatekey
Root: HKCU; Subkey: "Software\Classes\AppUserModelId\{#AppUserModelId}"; ValueType: none; Flags: uninsdeletekey dontcreatekey

; Nothing under %LOCALAPPDATA%\lumenna is ever removed: it is the person's store (and, Windows
; ignoring case, the same folder as the %LOCALAPPDATA%\Lumenna the notification icon is kept in).

[Run]
Filename: "{app}\lumenna.exe"; Description: "{cm:LaunchProgram,Lumenna}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; The sync daemon, if `lum daemon install` made one: a logon task that would go on starting a `lum`
; no longer there.
Filename: "{app}\lum.exe"; Parameters: "daemon uninstall"; Flags: runhidden; RunOnceId: "LumennaDaemon"

[Code]
function PathEntries(): String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Result) then
    Result := '';
end;

{ Whether the person's PATH lacks Dir, compared case-insensitively and whole entries only. }
function NeedsAddPath(Dir: String): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(PathEntries()) + ';') = 0;
end;

{ Takes Dir out of the person's PATH, wherever it is among the entries. }
procedure RemoveFromPath(Dir: String);
var
  Path: String;
  Found: Integer;
begin
  Path := ';' + PathEntries() + ';';
  Found := Pos(';' + Uppercase(Dir) + ';', Uppercase(Path));
  if Found = 0 then
    exit;
  Delete(Path, Found, Length(Dir) + 1);
  Path := Copy(Path, 2, Length(Path) - 2);
  RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
