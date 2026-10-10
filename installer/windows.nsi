; Windows installer for Tier List Maker (NSIS 3).
; Built by scripts/build-windows.sh, which passes:
;   -DVERSION=1.0.0 -DEXE=<path to tierlist-maker.exe> -DOUTFILE=<setup.exe> -DROOT=<project dir>
;
; Installs per user (no admin prompt) into %LOCALAPPDATA%\Programs\Tier List Maker.
; Uninstalling keeps the user's tier lists in %APPDATA%\Tier List Maker.

Unicode true
SetCompressor /SOLID lzma

!ifndef VERSION
  !error "Pass -DVERSION=x.y.z"
!endif
!ifndef EXE
  !error "Pass -DEXE=<path to tierlist-maker.exe>"
!endif
!ifndef OUTFILE
  !define OUTFILE "TierListMaker-${VERSION}-windows-setup.exe"
!endif
!ifndef ROOT
  !define ROOT ".."
!endif

!define APP_NAME "Tier List Maker"
!define PUBLISHER "Trabi"
!define EXE_NAME "Tier List Maker.exe"
!define REG_KEY "Software\TierListMaker"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\TierListMaker"

Name "${APP_NAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
InstallDirRegKey HKCU "${REG_KEY}" "InstallDir"
RequestExecutionLevel user
BrandingText "Made by ${PUBLISHER}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "Made by ${PUBLISHER}"
VIAddVersionKey "FileDescription" "${APP_NAME} Setup"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "${ROOT}\assets\app-icon.ico"
!define MUI_UNICON "${ROOT}\assets\app-icon.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${ROOT}\installer\welcome.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "${ROOT}\installer\welcome.bmp"

!define MUI_WELCOMEPAGE_TITLE "Welcome to ${APP_NAME}"
!define MUI_WELCOMEPAGE_TEXT "This will install ${APP_NAME} ${VERSION} on your computer.$\r$\n$\r$\nMake tier lists with your own tiers, colors and images. Your work is saved automatically, and you can export it as a PNG.$\r$\n$\r$\nNo administrator rights are needed.$\r$\n$\r$\nMade by ${PUBLISHER}."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE_NAME}"
!define MUI_FINISHPAGE_RUN_TEXT "Open ${APP_NAME} now"
!insertmacro MUI_PAGE_FINISH

!define MUI_UNCONFIRMPAGE_TEXT_TOP "${APP_NAME} will be removed. Your tier lists stay in $APPDATA\${APP_NAME} in case you install it again."
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "${APP_NAME}" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "/oname=${EXE_NAME}" "${EXE}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${EXE_NAME}"

  WriteRegStr HKCU "${REG_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${EXE_NAME}"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "The ${APP_NAME} app and a Start menu shortcut."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Add a shortcut to your desktop."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\${EXE_NAME}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "${REG_KEY}"
SectionEnd
