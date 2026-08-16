# The PowerShell twin of fetch-fonts.sh — see that file for why the variable
# fonts are instanced rather than registered directly.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
New-Item -ItemType Directory -Force -Path resources\fonts, build\fonts | Out-Null

python -c "import fontTools" 2>$null
if ($LASTEXITCODE -ne 0) {
    Write-Error "fontTools is missing. Install it with: python -m pip install fonttools"
}

$base = 'https://raw.githubusercontent.com/google/fonts/main/ofl'

function Get-Variable-Font($url, $name) {
    $out = "build\fonts\$name"
    if (-not (Test-Path $out)) { Write-Host "fetching $name"; Invoke-WebRequest -Uri $url -OutFile $out }
}

function New-Instance($source, $weight, $name) {
    $out = "resources\fonts\$name"
    if (Test-Path $out) { Write-Host "have $name"; return }
    Write-Host "instancing $name at wght=$weight"
    python -m fontTools.varLib.instancer "build\fonts\$source" "wght=$weight" -o $out | Out-Null
}

Get-Variable-Font "$base/onest/Onest%5Bwght%5D.ttf"         'Onest[wght].ttf'
Get-Variable-Font "$base/unbounded/Unbounded%5Bwght%5D.ttf" 'Unbounded[wght].ttf'

New-Instance 'Onest[wght].ttf'     500 'Onest-Medium.ttf'
New-Instance 'Onest[wght].ttf'     700 'Onest-Bold.ttf'
New-Instance 'Onest[wght].ttf'     800 'Onest-ExtraBold.ttf'
New-Instance 'Unbounded[wght].ttf' 800 'Unbounded-ExtraBold.ttf'

Get-ChildItem resources\fonts
