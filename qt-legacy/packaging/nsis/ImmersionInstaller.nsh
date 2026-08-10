!include "MUI2.nsh"
!include "nsDialogs.nsh"

Var ImmersionStartupCheckbox
Var ImmersionStartupEnabled

Function ImmersionStartupPageCreate
  !insertmacro MUI_HEADER_TEXT "Startup Options" "Choose whether Immersion runs at login"
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  ${NSD_CreateLabel} 0 0 100% 24u "Immersion can start automatically when you sign in to Windows."
  Pop $0

  ${NSD_CreateCheckbox} 0 32u 100% 14u "Launch Immersion when I log in to Windows"
  Pop $ImmersionStartupCheckbox
  ${NSD_Check} $ImmersionStartupCheckbox

  nsDialogs::Show
FunctionEnd

Function ImmersionStartupPageLeave
  ${NSD_GetState} $ImmersionStartupCheckbox $ImmersionStartupEnabled
FunctionEnd

!macro customPageAfterFiles
  Page custom ImmersionStartupPageCreate ImmersionStartupPageLeave
!macroend

!macro customInstall
  StrCmp $ImmersionStartupEnabled ${BST_CHECKED} 0 +3
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Immersion" "$INSTDIR\Immersion.exe"
    Goto +2
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Immersion"
!macroend

!macro customUnInstall
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Immersion"
!macroend
