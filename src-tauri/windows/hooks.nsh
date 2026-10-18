; Installer hooks for the Tauri NSIS template (docs/release.md).

; On a real uninstall (not when an update reinstalls):
; - "Start RightPeek when you sign in" (app::autostart) writes a per-user Run entry
;   named after the product; remove it so Windows does not try to start a
;   missing exe at sign-in. 0.1.0 could write it to HKLM where that key is
;   writable; deleting it there fails harmlessly elsewhere.
; - Windows itself records every exe that runs (Explorer app-switch counts,
;   Program Compatibility Assistant). Drop the values for our own paths; they
;   are per-user, so no admin rights are needed. C:\Windows\Prefetch is left
;   alone: it needs admin rights and Windows recycles it on its own.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
    DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FeatureUsage\AppSwitched" "$INSTDIR\${MAINBINARYNAME}.exe"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FeatureUsage\AppSwitched" "${BUNDLEID}"
    DeleteRegValue HKCU "Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Store" "$INSTDIR\${MAINBINARYNAME}.exe"
    DeleteRegValue HKCU "Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Store" "$INSTDIR\uninstall.exe"
  ${EndIf}
!macroend
