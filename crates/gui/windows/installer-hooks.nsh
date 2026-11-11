; NSIS installer hooks of Vitrine Studio (Tauri `bundle.windows.nsis.installerHooks`,
; ADR 0026). The installer puts the `vitrine.exe` command line tool next to
; the app (Tauri `externalBin`). On a fresh interactive install it offers to
; add the install folder to the user's PATH, so `vitrine` works in any
; terminal; the work is done by the CLI itself (`vitrine install-cli`, the
; same code as the app's "Install command line tool" action), which edits
; HKCU\Environment\Path without truncating it and broadcasts the change.
; The uninstaller removes the PATH entry again. Updates (/UPDATE) and
; passive/silent installs never ask and never touch PATH.

!macro NSIS_HOOK_POSTINSTALL
  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
    IfSilent vitrine_path_done
    MessageBox MB_YESNO|MB_ICONQUESTION "Add the Vitrine Studio command line tool (vitrine) to your PATH?$\r$\n$\r$\nYou can also do this later in Vitrine Studio: App settings > Command line tool." /SD IDNO IDNO vitrine_path_done
    ExecShellWait "" "$INSTDIR\vitrine.exe" "install-cli --yes" SW_HIDE
    vitrine_path_done:
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    IfFileExists "$INSTDIR\vitrine.exe" 0 vitrine_unpath_done
    ExecShellWait "" "$INSTDIR\vitrine.exe" "install-cli --remove --yes" SW_HIDE
    vitrine_unpath_done:
  ${EndIf}
!macroend
