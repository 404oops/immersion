; Immersion Windows installer. Standalone port of the old CPack/NSIS setup
; (packaging/nsis/ImmersionInstaller.nsh supplied the startup-options page;
; CPack generated the rest). Built by scripts/package-windows.ps1, which
; passes VERSION, EXE_SOURCE, and OUTFILE.

!include "MUI2.nsh"
!include "nsDialogs.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef EXE_SOURCE
  !define EXE_SOURCE "..\..\target\release\immersion.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "..\..\dist\ImmersionSetup-${VERSION}.exe"
!endif

!define APP_NAME "Immersion"
!define REG_UNINSTALL "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
!define REG_RUN "Software\Microsoft\Windows\CurrentVersion\Run"

Name "${APP_NAME}"
OutFile "${OUTFILE}"
Unicode True

; Per-user install: no admin prompt, and the app itself writes only HKCU.
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
InstallDirRegKey HKCU "Software\${APP_NAME}" "InstallDir"

!define MUI_ICON "..\..\crates\immersion\assets\icons\Immersion.ico"
!define MUI_UNICON "..\..\crates\immersion\assets\icons\Immersion.ico"

; Version block on the installer exe (requires x.x.x.x).
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "FileDescription" "${APP_NAME} Installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Copyright 404oops"

; ---- Pages -----------------------------------------------------------------

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
Page custom StartupPageCreate StartupPageLeave
!define MUI_FINISHPAGE_RUN "$INSTDIR\Immersion.exe"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; ---- Startup options page (ImmersionInstaller.nsh port) ---------------------

Var StartupCheckbox
Var StartupEnabled

Function StartupPageCreate
  !insertmacro MUI_HEADER_TEXT "Startup Options" "Choose whether Immersion runs at login"
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  ${NSD_CreateLabel} 0 0 100% 24u "Immersion can start automatically when you sign in to Windows."
  Pop $0

  ${NSD_CreateCheckbox} 0 32u 100% 14u "Launch Immersion when I log in to Windows"
  Pop $StartupCheckbox
  ${NSD_Check} $StartupCheckbox

  nsDialogs::Show
FunctionEnd

Function StartupPageLeave
  ${NSD_GetState} $StartupCheckbox $StartupEnabled
  ${If} $StartupEnabled == ${BST_CHECKED}
    WriteRegStr HKCU "${REG_RUN}" "${APP_NAME}" "$INSTDIR\Immersion.exe"
  ${Else}
    DeleteRegValue HKCU "${REG_RUN}" "${APP_NAME}"
  ${EndIf}
FunctionEnd

; ---- Install ----------------------------------------------------------------

Section "Immersion" SecMain
  SetOutPath "$INSTDIR"
  File "/oname=Immersion.exe" "${EXE_SOURCE}"

  WriteRegStr HKCU "Software\${APP_NAME}" "InstallDir" "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\Immersion.exe"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\Uninstall ${APP_NAME}.lnk" "$INSTDIR\Uninstall.exe"

  ; Add/Remove Programs entry.
  WriteRegStr HKCU "${REG_UNINSTALL}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${REG_UNINSTALL}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${REG_UNINSTALL}" "Publisher" "404oops"
  WriteRegStr HKCU "${REG_UNINSTALL}" "DisplayIcon" "$INSTDIR\Immersion.exe"
  WriteRegStr HKCU "${REG_UNINSTALL}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${REG_UNINSTALL}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${REG_UNINSTALL}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${REG_UNINSTALL}" "NoModify" 1
  WriteRegDWORD HKCU "${REG_UNINSTALL}" "NoRepair" 1
SectionEnd

; ---- Uninstall ---------------------------------------------------------------

Section "Uninstall"
  Delete "$INSTDIR\Immersion.exe"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\Uninstall ${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"

  ; customUnInstall port: drop the login entry.
  DeleteRegValue HKCU "${REG_RUN}" "${APP_NAME}"
  DeleteRegKey HKCU "${REG_UNINSTALL}"
  DeleteRegKey HKCU "Software\${APP_NAME}"
SectionEnd
