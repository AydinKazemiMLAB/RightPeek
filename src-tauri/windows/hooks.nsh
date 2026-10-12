; Installer hooks for the Tauri NSIS template (docs/PLAN.md Phase 4).

; "Start with Windows" (tauri-plugin-autostart) writes a per-user Run entry
; named after the product. Remove it on a real uninstall so Windows does not
; try to start a missing exe at sign-in; keep it when an update reinstalls.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
  ${EndIf}
!macroend
