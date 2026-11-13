# Draws the app's icon, the mark in the accent on a near-black rounded square, as assets/icon/app.ico
# for Windows (the program, the taskbar, and the installer) and assets/icon/app.png for Linux.
#
#   tools/make-icon.ps1
#
# The mark is the one in src/app/assets.rs, drawn on a 16-unit square; colors are src/app/theme.rs's
# BASE and ACCENT.
Add-Type -AssemblyName System.Drawing

$out = Join-Path $PSScriptRoot '..\assets\icon'
New-Item -ItemType Directory -Force $out | Out-Null

function Draw-Icon([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([System.Drawing.Color]::Transparent)
    $u = $size / 16.0
    # The square: the whole icon, with corners of 3 units.
    $r = 3 * $u
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc(0, 0, 2 * $r, 2 * $r, 180, 90)
    $path.AddArc($size - 2 * $r, 0, 2 * $r, 2 * $r, 270, 90)
    $path.AddArc($size - 2 * $r, $size - 2 * $r, 2 * $r, 2 * $r, 0, 90)
    $path.AddArc(0, $size - 2 * $r, 2 * $r, 2 * $r, 90, 90)
    $path.CloseFigure()
    $g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 0x08, 0x08, 0x0a))), $path)
    # The mark, scaled to sit inside the square with a margin of 2 of its units.
    $accent = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 0xdd, 0xc9, 0x4f))
    $m = 2 * $u; $k = 12 / 16.0
    foreach ($poly in @(@(2,14, 5,14, 9,2, 6,2), @(7,14, 10,14, 14,2, 11,2))) {
        $pts = for ($i = 0; $i -lt 8; $i += 2) { New-Object System.Drawing.PointF ([single]($m + $poly[$i] * $k * $u)), ([single]($m + $poly[$i + 1] * $k * $u)) }
        $g.FillPolygon($accent, [System.Drawing.PointF[]]$pts)
    }
    $g.Dispose()
    $bmp
}

# An .ico of PNG images, which every Windows since Vista reads.
$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$pngs = foreach ($s in $sizes) {
    $bmp = Draw-Icon $s
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    if ($s -eq 256) { $bmp.Save((Join-Path $out 'app.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    $bmp.Dispose()
    , $ms.ToArray()
}
$ico = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $ico
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $s = $sizes[$i]; $b = if ($s -ge 256) { 0 } else { $s }
    $w.Write([byte]$b); $w.Write([byte]$b); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]$pngs[$i].Length); $w.Write([uint32]$offset)
    $offset += $pngs[$i].Length
}
foreach ($p in $pngs) { $w.Write($p) }
$w.Flush()
[IO.File]::WriteAllBytes((Join-Path $out 'app.ico'), $ico.ToArray())
"wrote app.ico ($($sizes -join ', ') px) and app.png (256 px) to $out"
