using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

internal static class Program
{
    private const string ProjectDir = @"C:\Users\darkm\OneDrive\Documents\RobloxAI";
    private static readonly string LogPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Lumen",
        "launch.log");

    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] private static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] private static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int MessageBox(IntPtr hWnd, string text, string caption, uint type);

    [STAThread]
    private static void Main()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(LogPath));
            Log("click");

            if (FocusExistingApp())
            {
                Log("focused existing window");
                return;
            }

            var nodeDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "nodejs");
            var npm = Path.Combine(nodeDir, "npm.cmd");
            if (!File.Exists(npm))
            {
                Fail("npm introuvable. Installe Node.js LTS, puis réessaie.");
                return;
            }

            var cargoBin = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                ".cargo",
                "bin");
            var path = nodeDir + ";" + cargoBin + ";" + (Environment.GetEnvironmentVariable("PATH") ?? "");
            Environment.SetEnvironmentVariable("PATH", path);

            var start = new ProcessStartInfo
            {
                FileName = npm,
                Arguments = "run tauri dev",
                WorkingDirectory = ProjectDir,
                UseShellExecute = true,
                WindowStyle = ProcessWindowStyle.Minimized
            };
            Log("starting " + npm + " run tauri dev");
            Process.Start(start);
        }
        catch (Exception ex)
        {
            Fail(ex.Message);
        }
    }

    private static bool FocusExistingApp()
    {
        var self = Process.GetCurrentProcess().Id;
        var launcher = Process.GetCurrentProcess().MainModule != null
            ? Process.GetCurrentProcess().MainModule.FileName
            : "";

        foreach (var proc in Process.GetProcessesByName("lumen"))
        {
            if (proc.Id == self) continue;
            string path = "";
            try { path = proc.MainModule != null ? proc.MainModule.FileName : ""; }
            catch { /* access denied on some processes */ }
            if (!string.IsNullOrEmpty(launcher) &&
                string.Equals(path, launcher, StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }
            var handle = proc.MainWindowHandle;
            if (handle == IntPtr.Zero) continue;
            if (IsIconic(handle)) ShowWindowAsync(handle, 9);
            SetForegroundWindow(handle);
            return true;
        }
        return false;
    }

    private static void Fail(string message)
    {
        Log("error: " + message);
        MessageBox(IntPtr.Zero, message + "\n\nDétails : " + LogPath, "Lumen", 0x10);
    }

    private static void Log(string line)
    {
        try
        {
            File.AppendAllText(LogPath, DateTime.Now.ToString("s") + " " + line + Environment.NewLine, Encoding.UTF8);
        }
        catch { /* ignore */ }
    }
}
