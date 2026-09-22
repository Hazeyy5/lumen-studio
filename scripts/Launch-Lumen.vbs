Set sh = CreateObject("WScript.Shell")
ps = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File ""C:\Users\darkm\OneDrive\Documents\RobloxAI\scripts\Launch-Lumen.ps1"""
sh.Run ps, 0, False
