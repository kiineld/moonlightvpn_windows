# Draws crates/moonlight/assets/moonlight.ico from the app's own mark.
#
# The mark is `assets/logo-tile.svg`: a lime rounded square carrying a crescent
# and two stars. It is redrawn here with GDI+ rather than rasterised from the SVG
# so the build needs no image toolchain — the geometry is the same 44x44 box the
# Rust canvas uses, scaled per icon size.
#
# Run this by hand when the mark changes; the .ico is committed, because a build
# that generates its own icon needs a drawing library on every machine that
# builds it.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Set-Location (Join-Path $PSScriptRoot '..')

$out = 'crates\moonlight\assets\moonlight.ico'
New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null

# --ml-lime on the dark tile, with --ml-text-on-accent ink, so the icon reads the
# same as the mark inside the app.
$lime = [System.Drawing.Color]::FromArgb(255, 210, 255, 31)
$ink  = [System.Drawing.Color]::FromArgb(255, 16, 24, 40)

# Windows picks from these; 256 is what File Explorer's large views use.
$sizes = @(16, 24, 32, 48, 64, 128, 256)
$pngs = @()

foreach ($size in $sizes) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    $s = $size / 44.0
    # The slab: 10/44 of the width, matching the sidebar tile's radius.
    $r = [float]($size * 10.0 / 44.0)
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $r * 2
    $path.AddArc(0, 0, $d, $d, 180, 90)
    $path.AddArc($size - $d, 0, $d, $d, 270, 90)
    $path.AddArc($size - $d, $size - $d, $d, $d, 0, 90)
    $path.AddArc(0, $size - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    $brush = New-Object System.Drawing.SolidBrush $lime
    $g.FillPath($brush, $path)

    # The crescent, as a disc with a second disc taken out of it.
    #
    # Both circles are solved from the SVG's own arc pair rather than guessed:
    #   M30 22 a8.4 8.4 0 1 1 -9.4 -8.34 A10 10 0 0 0 30 22 Z
    # is an r=8.4 arc and an r=10 arc between (30,22) and (20.6,13.66), whose
    # centres work out at (21.6, 22) and (30.463, 12.011). Eyeballing the second
    # one — which an earlier pass did, putting it at (25.6, 17) with the wrong
    # radius — gives a crescent of the wrong thickness and angle, which is the
    # whole reason the icon did not look like the mark in the app.
    $crescent = New-Object System.Drawing.Drawing2D.GraphicsPath
    $crescent.AddEllipse([float]((21.6 - 8.4) * $s), [float]((22.0 - 8.4) * $s), [float](16.8 * $s), [float](16.8 * $s))
    $bite = New-Object System.Drawing.Drawing2D.GraphicsPath
    $bite.AddEllipse([float]((30.463 - 10.0) * $s), [float]((12.011 - 10.0) * $s), [float](20.0 * $s), [float](20.0 * $s))
    $region = New-Object System.Drawing.Region $crescent
    $region.Exclude($bite)
    $inkBrush = New-Object System.Drawing.SolidBrush $ink
    $g.FillRegion($inkBrush, $region)

    # The two stars, as in the SVG.
    foreach ($star in @(@(30.5, 12.5, 1.7), @(25.0, 8.0, 1.1))) {
        $cx = $star[0] * $s; $cy = $star[1] * $s; $rr = $star[2] * $s
        # Below ~24px the small star turns to mush and only muddies the mark.
        if ($size -lt 32 -and $star[2] -lt 1.5) { continue }
        $g.FillEllipse($inkBrush, [float]($cx - $rr), [float]($cy - $rr), [float]($rr * 2), [float]($rr * 2))
    }

    $g.Dispose()
    $png = Join-Path $env:TEMP "ml-icon-$size.png"
    $bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $pngs += $png
}

# Assemble a PNG-compressed .ico by hand: System.Drawing cannot write a
# multi-size icon, and every size above 48 is stored as PNG in a modern .ico.
$stream = [System.IO.File]::Create((Resolve-Path -LiteralPath (Split-Path $out) | ForEach-Object { Join-Path $_ (Split-Path $out -Leaf) }))
$writer = New-Object System.IO.BinaryWriter $stream
$writer.Write([UInt16]0)          # reserved
$writer.Write([UInt16]1)          # type: icon
$writer.Write([UInt16]$sizes.Count)

$offset = 6 + (16 * $sizes.Count)
$blobs = @()
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $bytes = [System.IO.File]::ReadAllBytes($pngs[$i])
    $blobs += ,$bytes
    $dim = $sizes[$i]
    $writer.Write([Byte]$(if ($dim -ge 256) { 0 } else { $dim }))
    $writer.Write([Byte]$(if ($dim -ge 256) { 0 } else { $dim }))
    $writer.Write([Byte]0)        # palette
    $writer.Write([Byte]0)        # reserved
    $writer.Write([UInt16]1)      # colour planes
    $writer.Write([UInt16]32)     # bits per pixel
    $writer.Write([UInt32]$bytes.Length)
    $writer.Write([UInt32]$offset)
    $offset += $bytes.Length
}
foreach ($bytes in $blobs) { $writer.Write($bytes) }
$writer.Flush(); $writer.Close(); $stream.Close()

Write-Output "wrote $out ($((Get-Item $out).Length) bytes, sizes: $($sizes -join ', '))"
