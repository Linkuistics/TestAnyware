@echo off
:: SetupComplete.cmd — Runs as SYSTEM after OOBE, before first user logon.
:: Installs VirtIO drivers (network + GPU), copies agent + desktop script from the
:: autounattend USB media, applies HKLM registry tweaks, and registers
:: the agent to start on logon via Task Scheduler.

set LOG=C:\Windows\Setup\Scripts\SetupComplete.log
echo [%date% %time%] SetupComplete.cmd starting >> %LOG%

:: 1. Find autounattend drive (look for testanyware-agent.exe marker)
set MEDIA_DRIVE=
if exist D:\testanyware-agent.exe set MEDIA_DRIVE=D:
if exist E:\testanyware-agent.exe set MEDIA_DRIVE=E:
if exist F:\testanyware-agent.exe set MEDIA_DRIVE=F:
if exist G:\testanyware-agent.exe set MEDIA_DRIVE=G:
if exist H:\testanyware-agent.exe set MEDIA_DRIVE=H:
echo [%date% %time%] MEDIA_DRIVE=%MEDIA_DRIVE% >> %LOG%
if "%MEDIA_DRIVE%"=="" (
    echo [%date% %time%] ERROR: Could not find autounattend drive >> %LOG%
    exit /b 1
)

:: 2. Install VirtIO drivers (network + GPU)
for %%D in (D E F G H) do (
    if exist %%D:\drivers\netkvm\netkvm.inf pnputil /add-driver %%D:\drivers\netkvm\netkvm.inf /install >> %LOG% 2>&1
    if exist %%D:\drivers\viogpu\viogpudo.inf pnputil /add-driver %%D:\drivers\viogpu\viogpudo.inf /install >> %LOG% 2>&1
)
echo [%date% %time%] VirtIO drivers installed >> %LOG%

:: 3. Copy agent binary
mkdir C:\testanyware
copy /y %MEDIA_DRIVE%\testanyware-agent.exe C:\testanyware\testanyware-agent.exe >> %LOG% 2>&1
echo [%date% %time%] Agent binary copied >> %LOG%

:: 4. Copy desktop-setup script (HKCU tweaks — runs via RunOnce in user session)
copy /y %MEDIA_DRIVE%\desktop-setup.ps1 C:\Windows\Setup\Scripts\desktop-setup.ps1 >> %LOG% 2>&1
echo [%date% %time%] Desktop setup script copied >> %LOG%

:: 5. Firewall rule for agent port
netsh advfirewall firewall add rule name="TestAnyware Agent" dir=in action=allow protocol=tcp localport=8648 >> %LOG% 2>&1
echo [%date% %time%] Firewall rule created >> %LOG%

:: 6. Register agent as Task Scheduler logon task (runs in user's desktop session)
schtasks /create /tn "TestAnywareAgent" /tr "C:\testanyware\testanyware-agent.exe" /sc onlogon /ru admin /f >> %LOG% 2>&1
echo [%date% %time%] Scheduled task created >> %LOG%

:: 7. Register desktop-setup.ps1 as RunOnce (fires on first interactive logon)
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce" /v DesktopSetup /t REG_SZ /d "powershell -ExecutionPolicy Bypass -File C:\Windows\Setup\Scripts\desktop-setup.ps1" /f >> %LOG% 2>&1

:: 8. HKLM registry tweaks — machine-wide desktop clutter removal
reg add "HKLM\SOFTWARE\Policies\Microsoft\Windows\Windows Search" /v AllowCortana /t REG_DWORD /d 0 /f >> %LOG% 2>&1
reg add "HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU" /v NoAutoRebootWithLoggedOnUsers /t REG_DWORD /d 1 /f >> %LOG% 2>&1
reg add "HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU" /v AUOptions /t REG_DWORD /d 2 /f >> %LOG% 2>&1
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v EnableFirstLogonAnimation /t REG_DWORD /d 0 /f >> %LOG% 2>&1
reg add "HKLM\SOFTWARE\Policies\Microsoft\Dsh" /v AllowNewsAndInterests /t REG_DWORD /d 0 /f >> %LOG% 2>&1
echo [%date% %time%] SetupComplete.cmd finished >> %LOG%
