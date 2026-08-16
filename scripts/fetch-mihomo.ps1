# Downloads the mihomo core and the Wintun driver into resources\.
#
# Neither is committed: mihomo is ~15 MB and Wintun ships as a signed DLL whose
# licence allows redistribution but not modification, so both are fetched at
# build time from their own releases.
#
# The geo databases are deliberately NOT fetched. mihomo downloads
# GeoSite.dat/GeoIP.dat on demand into %APPDATA%\Moonlight\core the first time a
# config references a geosite:/geoip: rule, which every panel config does. That
# costs one download on first connect and saves ~24 MB in the installer.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
New-Item -ItemType Directory -Force -Path resources\mihomo | Out-Null

$mihomoVersion = 'v1.19.29'
$wintunVersion = '0.14.1'

if (-not (Test-Path resources\mihomo\mihomo.exe)) {
    $name = "mihomo-windows-amd64-$mihomoVersion.zip"
    $url  = "https://github.com/MetaCubeX/mihomo/releases/download/$mihomoVersion/$name"
    Write-Host "fetching $name"
    Invoke-WebRequest -Uri $url -OutFile "$env:TEMP\$name"
    Expand-Archive -Path "$env:TEMP\$name" -DestinationPath "$env:TEMP\mihomo" -Force
    Get-ChildItem "$env:TEMP\mihomo" -Filter *.exe -Recurse |
        Select-Object -First 1 |
        ForEach-Object { Copy-Item $_.FullName resources\mihomo\mihomo.exe }
    $mihomoVersion | Out-File -Encoding ascii resources\mihomo\VERSION
}

# Wintun is what mihomo drives for TUN mode on Windows. Without this DLL beside
# the core, TUN fails at adapter creation with a message that names the DLL.
if (-not (Test-Path resources\mihomo\wintun.dll)) {
    $name = "wintun-$wintunVersion.zip"
    Write-Host "fetching $name"
    Invoke-WebRequest -Uri "https://www.wintun.net/builds/$name" -OutFile "$env:TEMP\$name"
    Expand-Archive -Path "$env:TEMP\$name" -DestinationPath "$env:TEMP\wintun" -Force
    Copy-Item "$env:TEMP\wintun\wintun\bin\amd64\wintun.dll" resources\mihomo\wintun.dll
}

Get-ChildItem resources\mihomo
