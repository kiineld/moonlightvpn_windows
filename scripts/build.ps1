# Builds the release binaries and lays out a portable folder in dist\.
#
# There is no installer step here. The portable layout is what the release
# workflow zips, and what the helper's --install reads from when the user turns
# TUN on: the service copies the core out of this folder into %ProgramData%,
# which is why mihomo.exe and wintun.dll sit beside the app rather than being
# embedded.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

& $PSScriptRoot\fetch-fonts.ps1
& $PSScriptRoot\fetch-mihomo.ps1
& $PSScriptRoot\fetch-flags.ps1
& $PSScriptRoot\fetch-geodata.ps1

cargo build --release --target x86_64-pc-windows-msvc

$dist = 'dist\Moonlight'
Remove-Item -Recurse -Force $dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $dist | Out-Null

$target = 'target\x86_64-pc-windows-msvc\release'
Copy-Item "$target\moonlight.exe"        $dist
Copy-Item "$target\moonlight-helper.exe" $dist
Copy-Item resources\mihomo\mihomo.exe    $dist
Copy-Item resources\mihomo\wintun.dll    $dist
Copy-Item LICENSE.md                     $dist

# The flags the server list draws. Loaded from disk at runtime rather than
# embedded, so 249 pictures cost nothing in the binary.
New-Item -ItemType Directory -Force -Path "$dist\flags" | Out-Null
Copy-Item resources\flags\*.png "$dist\flags"

# The geo databases. The helper stages these into %ProgramData% at install, and
# the app seeds its own copy from them, so neither core has to download anything
# on the connect that needs them.
New-Item -ItemType Directory -Force -Path "$dist\geodata" | Out-Null
Copy-Item resources\geodata\* "$dist\geodata"

Get-ChildItem $dist
