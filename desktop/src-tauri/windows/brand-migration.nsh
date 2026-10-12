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
  ${If} $VeiruneLegacyDirectory != ""
  ${AndIf} $VeiruneLegacyDirectory == $INSTDIR
  ${AndIf} ${FileExists} "$INSTDIR\fastcloud.exe"
    ; The new installer has now written its replacement uninstall registration.
    DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\Fastcloud"
    DeleteRegKey SHCTX "Software\fastcloud\Fastcloud"
    !insertmacro IsShortcutTarget "$SMPROGRAMS\Fastcloud.lnk" "$INSTDIR\fastcloud.exe"
    Pop $0
    ${If} $0 = 1
      Delete "$SMPROGRAMS\Fastcloud.lnk"
    ${EndIf}
    !insertmacro IsShortcutTarget "$DESKTOP\Fastcloud.lnk" "$INSTDIR\fastcloud.exe"
    Pop $0
    ${If} $0 = 1
      Delete "$DESKTOP\Fastcloud.lnk"
    ${EndIf}
  ${EndIf}
!macroend
