# Starts the app, saves a picture of its window, and closes it, without taking the focus or
# moving the mouse, so it can run while someone uses the PC.
#
#   tools/capture-window.ps1 -Out shot.png [-AppArgs "--demo host"] [-Click "298,177"] [-Hover "865,97"]
#                            [-Exe target/debug/cyberpunk-coop-tools.exe] [-Wait 2]
#
# -Click and -Hover take a point in the window's client area, in pixels at its DPI, and act there
# with posted mouse messages before the picture, so the real mouse never moves.
#
# PrintWindow with PW_RENDERFULLCONTENT draws the window even when other windows cover it, and the
# picture is the client area only, at the window's own DPI.
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Exe = "$PSScriptRoot\..\target\debug\cyberpunk-coop-tools.exe",
    [double]$Wait = 2,
    [string]$Title = 'Cyberpunk Co-op Tools',
    [string]$AppArgs = '',
    [string]$Click = '',
    [string]$Hover = ''
)

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;

public static class CoopCapture {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr v);

    public static void DpiAware() { SetProcessDpiAwarenessContext(new IntPtr(-4)); }

    public static IntPtr Find(uint pid, string title) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid || !IsWindowVisible(h)) return true;
            var s = new StringBuilder(256); GetWindowText(h, s, 256);
            if (s.ToString() == title) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static string Capture(IntPtr h, string path) {
        RECT wr; GetWindowRect(h, out wr);
        int w = wr.R - wr.L, ht = wr.B - wr.T;
        using (var bmp = new Bitmap(w, ht, PixelFormat.Format32bppArgb)) {
            using (var g = Graphics.FromImage(bmp)) {
                IntPtr hdc = g.GetHdc();
                PrintWindow(h, hdc, 2);
                g.ReleaseHdc(hdc);
            }
            RECT cr; GetClientRect(h, out cr);
            POINT p = new POINT(); ClientToScreen(h, ref p);
            var crop = new Rectangle(p.X - wr.L, p.Y - wr.T, cr.R, cr.B);
            crop.Intersect(new Rectangle(0, 0, w, ht));
            using (var c = bmp.Clone(crop, PixelFormat.Format32bppArgb)) c.Save(path, ImageFormat.Png);
            return string.Format("client {0}x{1} at {2} dpi", cr.R, cr.B, GetDpiForWindow(h));
        }
    }

    public static void Close(IntPtr h) { PostMessage(h, 0x0010, IntPtr.Zero, IntPtr.Zero); }

    static IntPtr XY(int x, int y) { return new IntPtr((y << 16) | (x & 0xFFFF)); }
    public static void Move(IntPtr h, int x, int y) { PostMessage(h, 0x0200, IntPtr.Zero, XY(x, y)); }
    public static void Click(IntPtr h, int x, int y) {
        PostMessage(h, 0x0200, IntPtr.Zero, XY(x, y));
        PostMessage(h, 0x0201, new IntPtr(1), XY(x, y));
        PostMessage(h, 0x0202, IntPtr.Zero, XY(x, y));
    }
}
"@
[CoopCapture]::DpiAware()

$start = @{ FilePath = (Resolve-Path $Exe).Path; PassThru = $true }
if ($AppArgs) { $start.ArgumentList = $AppArgs }
$p = Start-Process @start
try {
    $h = [IntPtr]::Zero
    for ($i = 0; $i -lt 80 -and $h -eq [IntPtr]::Zero; $i++) {
        Start-Sleep -Milliseconds 250
        $h = [CoopCapture]::Find([uint32]$p.Id, $Title)
    }
    if ($h -eq [IntPtr]::Zero) { throw "No window titled '$Title' appeared within 20s." }
    Start-Sleep -Seconds $Wait
    if ($Click) {
        $x, $y = $Click -split ',' | ForEach-Object { [int]$_ }
        [CoopCapture]::Click($h, $x, $y)
        Start-Sleep -Milliseconds 400
    }
    if ($Hover) {
        $x, $y = $Hover -split ',' | ForEach-Object { [int]$_ }
        # A tooltip waits for the pointer to rest, so the pointer arrives, moves once, and rests.
        [CoopCapture]::Move($h, $x - 2, $y)
        Start-Sleep -Milliseconds 100
        [CoopCapture]::Move($h, $x, $y)
        Start-Sleep -Milliseconds 1200
    }
    [CoopCapture]::Capture($h, [IO.Path]::GetFullPath($Out))
    [CoopCapture]::Close($h)
    if (-not $p.WaitForExit(5000)) { Stop-Process -Id $p.Id -Confirm:$false }
} finally {
    if (-not $p.HasExited) { Stop-Process -Id $p.Id -Confirm:$false }
}
