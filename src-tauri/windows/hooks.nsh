!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "$INSTDIR\resources\install-git.ps1"'
!macroend
