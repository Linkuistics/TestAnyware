param(
    [Parameter(Mandatory=$true, Position=0)]
    [string]$HexColor
)

if ($HexColor -notmatch '^[0-9A-Fa-f]{6}$') {
    Write-Error "Usage: set-wallpaper.ps1 <hex color e.g. 808080>"
    exit 1
}

$r = [Convert]::ToInt32($HexColor.Substring(0, 2), 16)
$g = [Convert]::ToInt32($HexColor.Substring(2, 2), 16)
$b = [Convert]::ToInt32($HexColor.Substring(4, 2), 16)

Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'WallPaper' -Value ''
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'WallpaperStyle' -Value '0'
Set-ItemProperty -Path 'HKCU:\Control Panel\Colors'  -Name 'Background' -Value "$r $g $b"

$signature = @'
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
public static extern bool SystemParametersInfoW(uint uiAction, uint uiParam, string pvParam, uint fWinIni);
'@

$type = Add-Type -MemberDefinition $signature -Name 'NativeMethods' -Namespace 'Win32' -PassThru

$SPI_SETDESKWALLPAPER = 0x0014
$SPIF_UPDATEINIFILE   = 0x01
$SPIF_SENDWININICHANGE = 0x02

$result = $type::SystemParametersInfoW(
    $SPI_SETDESKWALLPAPER,
    0,
    '',
    $SPIF_UPDATEINIFILE -bor $SPIF_SENDWININICHANGE
)

if (-not $result) {
    $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Write-Error "SystemParametersInfoW failed with error code $err"
    exit 1
}

Write-Host "Wallpaper set to solid color #$HexColor ($r $g $b)"
