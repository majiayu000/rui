# Opens examples in real Windows windows, drives them with SendInput and
# compares screenshots of the windows' client areas with the references in
# tests/windows_window/.
#
# Run with Windows PowerShell (its .NET has System.Drawing) in an interactive
# desktop session, after building the examples:
#   cargo build --no-default-features --features winit --example hello_world --example counter --example interaction_acceptance
#   powershell -File scripts/windows_window_pixels.ps1
# $env:RUI_PIXELS_UPDATE = "1" rewrites the references from this run.
$ErrorActionPreference = "Stop"

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { "target" }
$examples = Join-Path $targetDir "debug/examples"
$references = "tests/windows_window"
$out = if ($env:RUI_PIXELS_OUT) { $env:RUI_PIXELS_OUT } else { "target/windows-window-pixels" }
New-Item -ItemType Directory -Force -Path $out | Out-Null
$out = (Resolve-Path $out).Path
# A pixel differs when its color is farther than $fuzz (as a fraction of the
# largest RGB distance) from the reference, as ImageMagick's -fuzz on Linux; a
# window may have at most $maxDifferent such pixels.
$fuzz = if ($env:RUI_PIXELS_FUZZ) { [double]$env:RUI_PIXELS_FUZZ } else { 0.10 }
$maxDifferent = if ($env:RUI_PIXELS_MAX_DIFFERENT) { [int]$env:RUI_PIXELS_MAX_DIFFERENT } else { 150 }

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class WindowPixels {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hwnd, ref POINT point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);

    const uint SWP_NOSIZE = 0x1, SWP_NOMOVE = 0x2, SWP_NOZORDER = 0x4;

    /// The client area in screen pixels.
    public static Rectangle ClientArea(IntPtr hwnd) {
        RECT rect;
        GetClientRect(hwnd, out rect);
        POINT origin = new POINT();
        ClientToScreen(hwnd, ref origin);
        return new Rectangle(origin.X, origin.Y, rect.Right, rect.Bottom);
    }

    /// Put the window at the top left of the screen, above the others.
    public static void Raise(IntPtr hwnd) {
        SetWindowPos(hwnd, new IntPtr(-1), 0, 0, 0, 0, SWP_NOSIZE);
    }

    /// Give the window a client area of width x height pixels.
    public static void ResizeClient(IntPtr hwnd, int width, int height) {
        RECT outer;
        GetWindowRect(hwnd, out outer);
        Rectangle client = ClientArea(hwnd);
        int frameWidth = outer.Right - outer.Left - client.Width;
        int frameHeight = outer.Bottom - outer.Top - client.Height;
        SetWindowPos(hwnd, IntPtr.Zero, 0, 0, width + frameWidth, height + frameHeight, SWP_NOMOVE | SWP_NOZORDER);
    }

    public static void Click(int x, int y) {
        SetCursorPos(x, y);
        mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
        mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
    }

    public static void Key(byte vk) {
        keybd_event(vk, 0, 0, UIntPtr.Zero);
        keybd_event(vk, 0, 0x2, UIntPtr.Zero);
    }

    /// Press vk while modifier is held.
    public static void Chord(byte modifier, byte vk) {
        keybd_event(modifier, 0, 0, UIntPtr.Zero);
        Key(vk);
        keybd_event(modifier, 0, 0x2, UIntPtr.Zero);
    }

    public static void Capture(Rectangle area, string path) {
        using (Bitmap bitmap = new Bitmap(area.Width, area.Height, PixelFormat.Format24bppRgb))
        using (Graphics graphics = Graphics.FromImage(bitmap)) {
            graphics.CopyFromScreen(area.Location, Point.Empty, area.Size);
            bitmap.Save(path, ImageFormat.Png);
        }
    }

    static int[] Pixels(Bitmap bitmap) {
        Rectangle all = new Rectangle(0, 0, bitmap.Width, bitmap.Height);
        BitmapData data = bitmap.LockBits(all, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        int[] pixels = new int[bitmap.Width * bitmap.Height];
        Marshal.Copy(data.Scan0, pixels, 0, pixels.Length);
        bitmap.UnlockBits(data);
        return pixels;
    }

    /// How many pixels differ by more than fuzz, or -1 if the sizes differ.
    public static int Compare(string reference, string actual, double fuzz) {
        using (Bitmap expected = new Bitmap(reference))
        using (Bitmap captured = new Bitmap(actual)) {
            if (expected.Size != captured.Size) return -1;
            int[] a = Pixels(expected), b = Pixels(captured);
            double limit = fuzz * fuzz * 3 * 255 * 255;
            int different = 0;
            for (int i = 0; i < a.Length; i++) {
                double distance = 0;
                for (int shift = 0; shift < 24; shift += 8) {
                    int d = ((a[i] >> shift) & 0xff) - ((b[i] >> shift) & 0xff);
                    distance += d * d;
                }
                if (distance > limit) different++;
            }
            return different;
        }
    }
}
"@

# Work in physical pixels, as the winit window does.
[WindowPixels]::SetProcessDPIAware() | Out-Null

$script:failed = $false
$app = $null
$window = [IntPtr]::Zero

# Start example $name and wait until its window presented the first frame.
function Open-Example($name) {
    $log = Join-Path $out "$name.log"
    $env:RUST_LOG = "rui=info"
    $script:app = Start-Process -FilePath (Join-Path $examples "$name.exe") -RedirectStandardError $log -PassThru
    for ($i = 0; $i -lt 200; $i++) {
        $script:app.Refresh()
        if ($script:app.MainWindowHandle -ne [IntPtr]::Zero -and (Select-String -Quiet -Path $log -Pattern "winit window presenting through")) { break }
        Start-Sleep -Milliseconds 100
    }
    Select-String -Path $log -Pattern "winit window presenting through"
    $script:window = $script:app.MainWindowHandle
    if ($script:window -eq [IntPtr]::Zero) { throw "$name opened no window" }
    [WindowPixels]::Raise($script:window)
    Start-Sleep -Seconds 1
}

function Close-Example {
    Stop-Process -Id $script:app.Id
    $script:app.WaitForExit()
}

# Window coordinates in logical pixels, as the examples lay out.
function Scale {
    [WindowPixels]::GetDpiForWindow($script:window) / 96.0
}

function Click($x, $y) {
    $area = [WindowPixels]::ClientArea($script:window)
    $scale = Scale
    [WindowPixels]::Click($area.X + [int]($x * $scale), $area.Y + [int]($y * $scale))
    Start-Sleep -Milliseconds 200
}

# Compare the window with reference $name.png.
function Check($name) {
    $area = [WindowPixels]::ClientArea($script:window)
    # Park the pointer outside the window so no hover state shows.
    [WindowPixels]::SetCursorPos($area.Right + 50, $area.Bottom + 50) | Out-Null
    Start-Sleep -Seconds 1
    $area = [WindowPixels]::ClientArea($script:window)
    $actual = Join-Path $out "$name.png"
    $reference = Join-Path $references "$name.png"
    [WindowPixels]::Capture($area, $actual)
    if ($env:RUI_PIXELS_UPDATE -eq "1") {
        New-Item -ItemType Directory -Force -Path $references | Out-Null
        Copy-Item $actual $reference
        Write-Output "updated $reference"
        return
    }
    $different = [WindowPixels]::Compare((Resolve-Path $reference).Path, $actual, $fuzz)
    if ($different -ge 0 -and $different -le $maxDifferent) {
        Write-Output "ok ${name}: $different pixels differ"
    } else {
        Write-Output "FAIL ${name}: $different pixels differ, $($area.Width)x$($area.Height) captured (at most $maxDifferent may differ; -1 is a size mismatch; see $out)"
        $script:failed = $true
    }
}

Open-Example "hello_world"
Check "hello_world"
Close-Example

Open-Example "counter"
for ($i = 0; $i -lt 3; $i++) { Click 231 236 }
# The click focused the window; Return presses the focused "+" button.
[WindowPixels]::Key(0x0D)
Check "counter_input"
$scale = Scale
[WindowPixels]::ResizeClient($script:window, [int](500 * $scale), [int](340 * $scale))
Check "counter_resized"
Close-Example

# The system clipboard: paste text another app copied, then copy it back.
Open-Example "interaction_acceptance"
# A click on the empty bottom of the window focuses it.
Click 300 500
Set-Clipboard -Value "from-system"
# Tab focuses the first input; pasting twice shows the text came through the app.
$ctrl = 0x11
[WindowPixels]::Key(0x09)
foreach ($key in 0x56, 0x56, 0x41, 0x43) { [WindowPixels]::Chord($ctrl, $key) }
Start-Sleep -Seconds 1
$copied = Get-Clipboard -Raw
if ($copied -eq "from-systemfrom-system") {
    Write-Output "ok clipboard: pasted and copied through the system clipboard"
} else {
    Write-Output "FAIL clipboard: the system clipboard holds '$copied', expected 'from-systemfrom-system'"
    $script:failed = $true
}
Close-Example

if ($script:failed) { exit 1 }
