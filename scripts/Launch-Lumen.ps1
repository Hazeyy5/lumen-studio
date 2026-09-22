$ErrorActionPreference = "SilentlyContinue"
$proj = "C:\Users\darkm\OneDrive\Documents\RobloxAI"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class LumenWin {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
}
"@

function Show-Lumen {
  $proc = Get-Process -Name lumen -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } |
    Select-Object -First 1
  if (-not $proc) { return $false }
  if ([LumenWin]::IsIconic($proc.MainWindowHandle)) {
    [void][LumenWin]::ShowWindowAsync($proc.MainWindowHandle, 9)
  }
  [void][LumenWin]::SetForegroundWindow($proc.MainWindowHandle)
  return $true
}

if (Show-Lumen) { exit 0 }

Set-Location $proj
$npmCmd = Get-Command npm.cmd -ErrorAction SilentlyContinue
$npm = if ($npmCmd) { $npmCmd.Source } else { "npm.cmd" }
Start-Process -FilePath $npm -ArgumentList @("run", "tauri", "dev") -WorkingDirectory $proj -WindowStyle Hidden
exit 0
