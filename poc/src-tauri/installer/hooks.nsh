; Tauri CLI 2.12.4's standard uninstall page offers app-data deletion.
; Keep every library on uninstall/reinstall; cleaning a library is an explicit user action.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
    MessageBox MB_OK "Prompt Shelf 卸载会保留资料库、图片与备份。重新安装后可继续使用。" /SD IDOK
  ${EndIf}
  StrCpy $DeleteAppDataCheckboxState 0
!macroend
