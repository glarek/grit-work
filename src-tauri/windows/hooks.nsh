; Current-user autostart for the first Windows release.
; The Run value name must match autostart.rs WINDOWS_AUTOSTART_RUN_VALUE
; Use the fork product name so upstream Work Review autostart is untouched.
; Tauri itself only removes HKCU Run\${PRODUCTNAME} and skips that on /UPDATE.

Var GritPrevInstall
Var GritExistingRun
Var GritUserDisabled
Var GritOcrExit
Var GritOcrPowerShell
Var GritOcrArguments

!include "x64.nsh"
!define GRIT_OCR_SCRIPT "${__FILEDIR__}\install-ocr-languages.ps1"

!macro GRIT_ENSURE_OCR_LANGUAGES
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=install-ocr-languages.ps1 "${GRIT_OCR_SCRIPT}"
  StrCpy $GritOcrPowerShell "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  ${If} ${RunningX64}
    StrCpy $GritOcrPowerShell "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  ${EndIf}
  StrCpy $GritOcrArguments ""
  ${If} ${Silent}
    ; Unattended deployment must already be elevated if a language is missing.
    StrCpy $GritOcrArguments "-NoElevation"
  ${EndIf}
  DetailPrint "Kontrollerar svenska och engelska OCR-språk. Hämtning via Windows Update kan ta flera minuter."
  nsExec::ExecToLog '$\"$GritOcrPowerShell$\" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $\"$PLUGINSDIR\install-ocr-languages.ps1$\" $GritOcrArguments'
  Pop $GritOcrExit
  ${If} $GritOcrExit == 3010
    SetRebootFlag true
    DetailPrint "Starta om Windows för att slutföra installationen av OCR-språken."
  ${ElseIf} $GritOcrExit != 0
    IfSilent +2
      MessageBox MB_OK|MB_ICONSTOP "Svenska och engelska OCR-språk kunde inte installeras (kod $GritOcrExit). Kontrollera internetanslutning och Windows Update och tillåt administratörssteget. Kör sedan installationen igen."
    SetErrorLevel 1
    Abort "OCR-språken saknas. Installationen avbröts."
  ${EndIf}
  SetOutPath "$INSTDIR"
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro GRIT_ENSURE_OCR_LANGUAGES
  StrCpy $GritPrevInstall ""
  StrCpy $GritExistingRun ""
  StrCpy $GritUserDisabled 0
  ReadRegStr $GritPrevInstall SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" "DisplayVersion"
  ReadRegStr $GritExistingRun HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "GRIT Work"
  ClearErrors
  ReadRegDWORD $GritUserDisabled HKCU "Software\se.grit.work" "AutostartUserDisabled"
  IfErrors 0 +2
    StrCpy $GritUserDisabled 0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Explicit opt-out wins over a fresh install and over an upgrade.
  ${If} $GritUserDisabled = 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "GRIT Work"
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
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "GRIT Work" "$0"
  ${ElseIf} $GritPrevInstall != ""
    ; Installed before, and the Run value is absent: user disabled it.
  ${Else}
    ; First install. Silent current-user startup is the default.
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "GRIT Work" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --autostart --hidden"
  ${EndIf}
  ; Notify Explorer after replacing the executable's icon resources.
  ; Do not remove the user's pinned shortcuts or global icon cache.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; /UPDATE uninstall must leave the Run value and the opt-out marker.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "GRIT Work"
    DeleteRegValue HKCU "Software\se.grit.work" "AutostartUserDisabled"
    DeleteRegKey /ifempty HKCU "Software\se.grit.work"
  ${EndIf}
!macroend
