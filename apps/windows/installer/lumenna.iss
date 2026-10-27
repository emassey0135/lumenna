; Lumenna's Windows installer: the app and `lum` together, for one person, with no administrator.
;
; Built by the release workflow (`.github/workflows/release.yml`), once per architecture:
;
;   iscc /DArch=x64 /DSource=<folder with lumenna.exe and lum.exe>
;        /DOutput=<folder> apps\windows\installer\lumenna.iss
;
; The two share one store as they always do, the profile's own (%LOCALAPPDATA%\lumenna), and `lum`
; is put on the person's PATH. Unsigned, as winget accepts and a direct download allows.

; What Lumenna is, read from the workspace's Cargo.toml (`[workspace.metadata.lumenna]`) as the
; installer is built, so its version and name are changed there alone. ReadIni reads a TOML table
; of plain strings as an INI section, taking the quotes off.
#define Workspace AddBackslash(SourcePath) + "..\..\..\Cargo.toml"
#define Name ReadIni(Workspace, "workspace.metadata.lumenna", "name")
#define AppVersion ReadIni(Workspace, "workspace.metadata.lumenna", "version")
#define Publisher ReadIni(Workspace, "workspace.metadata.lumenna", "publisher")
#define Homepage ReadIni(Workspace, "workspace.metadata.lumenna", "homepage")
#define Issues ReadIni(Workspace, "workspace.metadata.lumenna", "issues")
; The notification identity the app gives itself (toast.rs), which the Start menu shortcut carries
; so Windows ties the app's notifications to it.
#define AppUserModelId ReadIni(Workspace, "workspace.metadata.lumenna", "id")
#if Name == "" || AppVersion == "" || Publisher == "" || Homepage == "" || Issues == "" || AppUserModelId == ""
  #error The workspace's Cargo.toml does not say all of what Lumenna is ([workspace.metadata.lumenna])
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

[Setup]
; Never to change: Windows knows an upgrade, and winget an installed copy, by it.
AppId={{4FFFD1BB-2C81-432A-9E26-1F7935D5B7D0}
AppName={#Name}
AppVersion={#AppVersion}
AppVerName={#Name} {#AppVersion}
AppPublisher={#Publisher}
AppPublisherURL={#Homepage}
AppSupportURL={#Issues}
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
UninstallDisplayName={#Name}
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
Name: "{autoprograms}\{#Name}"; Filename: "{app}\lumenna.exe"; AppUserModelID: "{#AppUserModelId}"
Name: "{autodesktop}\{#Name}"; Filename: "{app}\lumenna.exe"; AppUserModelID: "{#AppUserModelId}"; Tasks: desktopicon

[Registry]
; `lum` on the person's PATH; taken off again on uninstalling (RemoveFromPath, below).
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))
; What the app writes itself, taken away with it: opening at sign-in (settings.rs's Run value, the
; usual profile's; every other profile's has a name of its own, which `lumenna.exe --uninstall`
; takes away, below) and its notification identity (toast.rs). Written by the app, so created by
; nothing here.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Lumenna"; Flags: uninsdeletevalue dontcreatekey
Root: HKCU; Subkey: "Software\Classes\AppUserModelId\{#AppUserModelId}"; ValueType: none; Flags: uninsdeletekey dontcreatekey

; %LOCALAPPDATA%\lumenna, the store, its backups and the notification icon, is removed only when the
; person says so as they uninstall (DeleteData, below), never by a silent uninstall such as
; winget's, nor by an upgrade, which does not uninstall.

[Run]
Filename: "{app}\lumenna.exe"; Description: "{cm:LaunchProgram,{#Name}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; What the app wrote that only it can name: every profile's value opening it at sign-in, and the
; Jump List the shell keeps for the notification identity, whose tasks would start a program no
; longer there.
Filename: "{app}\lumenna.exe"; Parameters: "--uninstall"; Flags: runhidden waituntilterminated; RunOnceId: "LumennaForget"
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

{ The person's data on this PC, when they choose: the store (the profile's default,
  %LOCALAPPDATA%\lumenna\data), its automatic backups beside it, the notification icon, and the
  app's own settings (the shortcuts from anywhere). Asked only when someone is there to answer,
  No the default; a backup folder chosen elsewhere is left where it is. }
procedure DeleteData();
begin
  if UninstallSilent() then
    exit;
  if MsgBox('Also delete your Lumenna data on this PC: your tasks, settings and automatic backups?' + #13#10#13#10 +
      'Devices you have paired keep their own copies. If this PC is your only one, everything is lost. ' +
      'Choose No to keep it for a later install.', mbConfirmation, MB_YESNO or MB_DEFBUTTON2) <> IDYES then
    exit;
  DelTree(ExpandConstant('{localappdata}\lumenna'), True, True, True);
  RegDeleteKeyIncludingSubkeys(HKEY_CURRENT_USER, 'Software\Lumenna');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveFromPath(ExpandConstant('{app}'));
    DeleteData();
  end;
end;
