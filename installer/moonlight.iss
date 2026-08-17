; Moonlight VPN — Windows installer.
;
; Built by the release workflow with Inno Setup, which ships on the GitHub
; windows runner image. Everything the app needs lives in one directory: the
; client, the privileged helper, the mihomo core and the Wintun driver. Shipping
; the app alone is what left people with a TUN button that had no service binary
; to elevate.
;
; The helper service is registered *here* rather than from the app's own
; Установить службу button. Setup is already elevated, so it costs no second UAC
; prompt, and it means TUN works on first run instead of after a detour through
; Settings. The button stays for anyone who declines the task or installs from
; the zip.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "Moonlight"
#define AppPublisher "Moonlight"
#define AppURL "https://github.com/kiineld/moonlightvpn_windows"
#define AppExe "moonlight.exe"
#define HelperExe "moonlight-helper.exe"

[Setup]
; Never change this GUID: it is how Windows recognises an upgrade rather than a
; second parallel installation.
AppId={{8F31C2A7-4B6E-4E2B-9E5D-2C1A7F0B6D34}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE.md
OutputDir=..\installer-out
OutputBaseFilename=Moonlight-Setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; The helper is a system service and the app installs under Program Files, so
; setup needs elevation. Asking once here is the whole point of shipping an
; installer.
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; 1809 is the floor the README states, and matches what wintun and the core need.
MinVersion=10.0.17763
UninstallDisplayIcon={app}\{#AppExe}
SetupIconFile=..\crates\moonlight\assets\moonlight.ico
; Setup can neither replace nor delete a running executable. Inno detects the
; files in use and offers to close the app, which is a far better failure than
; a "file in use" dialog halfway through copying.
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked
Name: "tunservice"; Description: "{cm:InstallTunService}"

[Files]
Source: "..\dist\Moonlight\{#AppExe}";    DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Moonlight\{#HelperExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Moonlight\mihomo.exe";   DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Moonlight\wintun.dll";   DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Moonlight\LICENSE.md";   DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Moonlight\flags\*.png";  DestDir: "{app}\flags"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}";        Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}";  Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
; Registered from the elevated installer, so the user is not asked twice.
;
; The exit code is checked. It was ignored, so a `--install` that registered the
; service and then failed to start it finished the install silently and left TUN
; broken until the next reboot — which is exactly what happened on a second
; install, where creating an already-existing service failed before the line
; that starts it.
Filename: "{app}\{#HelperExe}"; Parameters: "--install"; StatusMsg: "{cm:RegisteringService}"; Flags: runhidden waituntilterminated; Tasks: tunservice; AfterInstall: CheckServiceInstalled
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Before the files go, or the service is left pointing at a binary that no
; longer exists and Windows keeps it registered until the next reboot.
Filename: "{app}\{#HelperExe}"; Parameters: "--uninstall"; Flags: runhidden waituntilterminated; RunOnceId: "RemoveTunService"

[CustomMessages]
en.InstallTunService=Install the TUN helper service (needed for TUN mode)
ru.InstallTunService=Установить службу для режима TUN
en.RegisteringService=Registering the TUN helper service...
ru.RegisteringService=Регистрация службы TUN...
en.ServiceFailed=The TUN helper service could not be started. Everything else is installed and system-proxy mode will work; turn TUN on from Settings to try again.
ru.ServiceFailed=Не удалось запустить службу TUN. Всё остальное установлено, режим системного прокси работает; включите TUN в настройках, чтобы попробовать снова.

[Code]
{ Inno ignores a [Run] entry's exit code, so a helper that registered the
  service but could not start it used to finish the install silently. Saying so
  is the difference between "TUN does not work" and "TUN does not work *and
  nobody mentioned it*". It is a warning rather than a failed install: the rest
  of the app is fine without the service. }
procedure CheckServiceInstalled();
var
  ResultCode: Integer;
begin
  if not Exec(ExpandConstant('{sys}\sc.exe'), 'query MoonlightHelper', '',
              SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
    MsgBox(ExpandConstant('{cm:ServiceFailed}'), mbError, MB_OK);
end;
