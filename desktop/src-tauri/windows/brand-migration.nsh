; Product identity and fastcloud.exe stay stable. Reuse the previous NSIS
; installation directory when changing the visible product name to Veirune.
; No user data, music, credentials or registry outside these app keys is removed.
Var VeiruneLegacyDirectory

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $VeiruneLegacyDirectory SHCTX "Software\fastcloud\Fastcloud" ""
  ${If} $VeiruneLegacyDirectory != ""
  ${AndIf} ${FileExists} "$VeiruneLegacyDirectory\fastcloud.exe"
    ${If} $UpdateMode = 1
    ${OrIf} $INSTDIR == "$LOCALAPPDATA\${PRODUCTNAME}"
      StrCpy $INSTDIR $VeiruneLegacyDirectory
      SetOutPath $INSTDIR
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Tauri skips new shortcut names in /UPDATE mode. Repair the Start Menu entry
  ; explicitly, and replace legacy desktop links before removing either old link.
  ${If} ${FileExists} "$INSTDIR\fastcloud.exe"
    ${If} $NoShortcutMode <> 1
      ${IfNot} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
        CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\fastcloud.exe"
        !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      ${EndIf}
      ${If} $VeiruneLegacyDirectory != ""
      ${AndIf} $VeiruneLegacyDirectory == $INSTDIR
        ${IfNot} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
          CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\fastcloud.exe"
          !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
        ${EndIf}
        !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\fastcloud.exe"
        Pop $0
        ${If} $0 = 1
          !insertmacro IsShortcutTarget "$SMPROGRAMS\Fastcloud.lnk" "$INSTDIR\fastcloud.exe"
          Pop $0
          ${If} $0 = 1
            Delete "$SMPROGRAMS\Fastcloud.lnk"
          ${EndIf}
        ${EndIf}
        !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\fastcloud.exe"
        Pop $0
        ${If} $0 = 1
          !insertmacro IsShortcutTarget "$DESKTOP\Fastcloud.lnk" "$INSTDIR\fastcloud.exe"
          Pop $0
          ${If} $0 = 1
            Delete "$DESKTOP\Fastcloud.lnk"
          ${EndIf}
        ${EndIf}
      ${EndIf}
    ${EndIf}
    ${If} $VeiruneLegacyDirectory != ""
    ${AndIf} $VeiruneLegacyDirectory == $INSTDIR
      ; Registration cleanup happens only after the replacement is installed.
      DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\Fastcloud"
      DeleteRegKey SHCTX "Software\fastcloud\Fastcloud"
    ${EndIf}
  ${EndIf}
!macroend
