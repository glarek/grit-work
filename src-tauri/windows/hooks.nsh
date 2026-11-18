; Current-user autostart for the first Windows release.
; The Run value name must match autostart.rs WINDOWS_AUTOSTART_RUN_VALUE
; (Cargo package name / package_info().name), not the product display name.
; Tauri itself only removes HKCU Run\${PRODUCTNAME} and skips that on /UPDATE.

Var GritPrevInstall
Var GritExistingRun
Var GritUserDisabled

!macro NSIS_HOOK_PREINSTALL
  StrCpy $GritPrevInstall ""
  StrCpy $GritExistingRun ""
  StrCpy $GritUserDisabled 0
  ReadRegStr $GritPrevInstall SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" "DisplayVersion"
  ReadRegStr $GritExistingRun HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "work-review"
  ClearErrors
  ReadRegDWORD $GritUserDisabled HKCU "Software\se.grit.work" "AutostartUserDisabled"
  IfErrors 0 +2
    StrCpy $GritUserDisabled 0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Explicit opt-out wins over a fresh install and over an upgrade.
  ${If} $GritUserDisabled = 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "work-review"
  ${ElseIf} $GritExistingRun != ""
    ; Upgrade or reinstall while autostart is on: point at the new EXE.
    ; Keep --hidden only when the previous command already had it.
    StrCpy $0 "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --autostart"
    ClearErrors
    ${StrLoc} $1 $GritExistingRun "--hidden" ">"
    ${IfNot} ${Errors}
      StrCpy $0 "$0 --hidden"
    ${Else}
      ClearErrors
      ${StrLoc} $1 $GritExistingRun "--minimized" ">"
      ${IfNot} ${Errors}
        StrCpy $0 "$0 --hidden"
      ${EndIf}
    ${EndIf}
    ClearErrors
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "work-review" "$0"
  ${ElseIf} $GritPrevInstall != ""
    ; Installed before, and the Run value is absent: user disabled it.
  ${Else}
    ; First install. Silent current-user startup is the default.
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "work-review" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --autostart --hidden"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; /UPDATE uninstall must leave the Run value and the opt-out marker.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "work-review"
    DeleteRegValue HKCU "Software\se.grit.work" "AutostartUserDisabled"
    DeleteRegKey /ifempty HKCU "Software\se.grit.work"
  ${EndIf}
!macroend
