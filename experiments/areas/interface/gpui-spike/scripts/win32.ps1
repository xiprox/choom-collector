# Small Win32 helpers for the spike: capture a window with PrintWindow (works when occluded,
# no focus change) and post synthetic mouse messages to it (the real cursor is not moved).
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class SpikeWin {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);

    public static void DpiAware() { SetProcessDpiAwarenessContext(new IntPtr(-4)); }

    // Capture the client area only, via PrintWindow(PW_RENDERFULLCONTENT).
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
            return string.Format("window {0}x{1}, client {2}x{3}, dpi {4}", w, ht, cr.R, cr.B, GetDpiForWindow(h));
        }
    }

    static IntPtr XY(int x, int y) { return new IntPtr((y << 16) | (x & 0xFFFF)); }
    public static void Move(IntPtr h, int x, int y) { PostMessage(h, 0x0200, IntPtr.Zero, XY(x, y)); }
    public static void Click(IntPtr h, int x, int y) {
        PostMessage(h, 0x0200, IntPtr.Zero, XY(x, y));
        PostMessage(h, 0x0201, new IntPtr(1), XY(x, y));
        PostMessage(h, 0x0202, IntPtr.Zero, XY(x, y));
    }
    public static void Close(IntPtr h) { PostMessage(h, 0x0010, IntPtr.Zero, IntPtr.Zero); }
}
"@
[SpikeWin]::DpiAware()
