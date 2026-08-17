# Downloads the geo databases the core needs to parse a panel config.
#
# Every Remnawave config carries GEOSITE and GEOIP rules, and mihomo resolves
# those while *parsing* — downloading the databases itself if it has none, using
# its own half-configured resolver before any tunnel exists. That download fails
# on plenty of networks, and when it does the failure is fatal: the core exits
# without binding its API and all the app can report is silence.
#
# The app fetches them for its own core, but the TUN core runs as LocalSystem out
# of %ProgramData%, which the app cannot write to. So they ship with the build
# and the helper stages them at install.
#
# GEOIP is the MMDB, not geoip.dat: mihomo only reads the .dat form when a config
# sets `geodata-mode: true`.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
New-Item -ItemType Directory -Force -Path resources\geodata | Out-Null

$base = 'https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest'
$files = @(
    @{ Name = 'GeoSite.dat';   Url = "$base/geosite.dat" },
    @{ Name = 'geoip.metadb';  Url = "$base/geoip.metadb" }
)

foreach ($file in $files) {
    $out = "resources\geodata\$($file.Name)"
    # 100 KB is the floor the app uses too: a cached error page is present,
    # small, and fatal to every connect until somebody deletes it by hand.
    if ((Test-Path -LiteralPath $out) -and (Get-Item -LiteralPath $out).Length -gt 100KB) {
        Write-Host "have $($file.Name)"
        continue
    }
    Write-Host "fetching $($file.Name)"
    $temp = "$out.part"
    Invoke-WebRequest -Uri $file.Url -OutFile $temp -UseBasicParsing
    Move-Item -LiteralPath $temp -Destination $out -Force
}

Get-ChildItem resources\geodata | Select-Object Name, Length
