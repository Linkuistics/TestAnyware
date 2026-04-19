# desktop-setup.ps1 — Runs via RunOnce in the desktop session during golden
# image creation. Sets wallpaper, disables visual clutter, and signals
# completion via a marker file. All HKCU writes are reliable here because
# this runs in the interactive user session, not over SSH.

# --- Wallpaper: solid gray ---
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name WallPaper -Value ''
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name WallpaperStyle -Value '0'
Set-ItemProperty -Path 'HKCU:\Control Panel\Colors' -Name Background -Value '128 128 128'
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Wallpapers' -Name BackgroundType -Value 1 -PropertyType DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' -Name EnableTransparency -Value 0

# Disable Windows Spotlight (overrides wallpaper with rotating images)
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name RotatingLockScreenEnabled -Value 0 -PropertyType DWord -Force
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name RotatingLockScreenOverlayEnabled -Value 0 -PropertyType DWord -Force
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name ContentDeliveryAllowed -Value 0 -PropertyType DWord -Force

# Apply wallpaper via Win32 API (takes effect immediately in desktop session)
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class Wallpaper {
    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    public static extern int SystemParametersInfo(int uAction, int uParam, string lpvParam, int fuWinIni);
}
"@
[Wallpaper]::SystemParametersInfo(0x0014, 0, '', 3)

# --- Taskbar: disable chat, copilot, search box ---
# Widgets are disabled via HKLM Group Policy (Dsh\AllowNewsAndInterests=0)
# because Explorer protects the HKCU TaskbarDa key from writes.
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name TaskbarMn -Value 0 -PropertyType DWord -Force
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name ShowCopilotButton -Value 0 -PropertyType DWord -Force
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name SearchboxTaskbarMode -Value 0 -PropertyType DWord -Force

# --- Disable notifications ---
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\PushNotifications' -Name ToastEnabled -Value 0 -PropertyType DWord -Force

# --- Disable search highlights ---
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\SearchSettings' -Name IsDynamicSearchBoxEnabled -Value 0 -PropertyType DWord -Force

# --- Disable suggested content / first-run experience ---
$cdm = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'
@(338389, 310093, 338388, 338393, 353694, 353696) | ForEach-Object {
    New-ItemProperty -Path $cdm -Name "SubscribedContent-${_}Enabled" -Value 0 -PropertyType DWord -Force
}

# --- Disable "Let's finish setting up your device" nag ---
$engPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement'
if (-not (Test-Path $engPath)) { New-Item -Path $engPath -Force }
New-ItemProperty -Path $engPath -Name ScoobeSystemSettingEnabled -Value 0 -PropertyType DWord -Force

# Signal completion
New-Item -Path 'C:\Windows\Setup\Scripts\desktop-setup-done.txt' -Force
