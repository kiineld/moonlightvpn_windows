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

# Downloads with backoff.
#
# raw.githubusercontent.com rate-limits by IP, and a shared CI runner can arrive
# already throttled by somebody else's job — a 429 on the first attempt says
# nothing about this build. GITHUB_TOKEN, when the workflow provides one, raises
# the limit well clear of it.
function Get-Remote($url, $out) {
    if (Test-Path $out) { Write-Host "have $(Split-Path $out -Leaf)"; return }
    $headers = @{ 'User-Agent' = 'moonlight-build' }
    if ($env:GITHUB_TOKEN) { $headers['Authorization'] = "Bearer $($env:GITHUB_TOKEN)" }

    $delays = @(0, 3, 8, 20, 45)
    for ($i = 0; $i -lt $delays.Count; $i++) {
        if ($delays[$i] -gt 0) {
            Write-Host "  retrying in $($delays[$i])s"
            Start-Sleep -Seconds $delays[$i]
        }
        try {
            Write-Host "fetching $(Split-Path $out -Leaf)"
            Invoke-WebRequest -Uri $url -OutFile $out -Headers $headers -UseBasicParsing
            return
        } catch {
            Write-Host "  $($_.Exception.Message)"
            if ($i -eq $delays.Count - 1) { throw }
        }
    }
}

function Get-Variable-Font($url, $name) {
    Get-Remote $url "build\fonts\$name"
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
