!macro NSIS_HOOK_POSTINSTALL
  SetOutPath "$INSTDIR"
  File /a "/oname=WebView2Loader.dll" "${__FILEDIR__}\..\..\WebView2Loader.dll"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$INSTDIR\WebView2Loader.dll"
!macroend
