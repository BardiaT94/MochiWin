; MochiSetup.exe: installs dist-win/ for the current user, no admin rights needed.
; Built by tools/win-build.sh: makensis -DVERSION=x.y.z packaging/windows/mochi.nsi
; Paths are relative to this file's folder (makensis works from there).

Unicode true
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define SRC "..\..\dist-win"
!define UNINST "Software\Microsoft\Windows\CurrentVersion\Uninstall\Mochi"

Name "Mochi"
OutFile "..\..\MochiSetup.exe"
InstallDir "$LOCALAPPDATA\Programs\Mochi"
InstallDirRegKey HKCU "${UNINST}" "InstallLocation"
RequestExecutionLevel user
BrandingText "Mochi ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Mochi"
VIAddVersionKey "FileDescription" "Mochi Setup"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

!define MUI_ICON "..\..\assets\mochi.ico"
!define MUI_UNICON "..\..\assets\mochi.ico"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_NODESC
!define MUI_FINISHPAGE_RUN "$INSTDIR\mochi.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start Mochi"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Mochi" SecMain
  SectionIn RO
  ; A running Mochi keeps its files locked: ask for it to be closed first.
  running:
  IfFileExists "$INSTDIR\mochi.exe" 0 free
  ClearErrors
  FileOpen $0 "$INSTDIR\mochi.exe" a
  IfErrors 0 +3
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Mochi is running. Close it, then click Retry." IDRETRY running
    Abort
  FileClose $0
  free:

  SetOutPath "$INSTDIR"
  File /r "${SRC}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Mochi.lnk" "$INSTDIR\mochi.exe"

  WriteRegStr HKCU "${UNINST}" "DisplayName" "Mochi"
  WriteRegStr HKCU "${UNINST}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST}" "Publisher" "Mochi"
  WriteRegStr HKCU "${UNINST}" "DisplayIcon" "$INSTDIR\mochi.exe,0"
  WriteRegStr HKCU "${UNINST}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST}" "EstimatedSize" "$0"
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\Mochi.lnk" "$INSTDIR\mochi.exe"
SectionEnd

; Only Mochi's own files go: a folder chosen by hand may hold other things. Settings in
; %APPDATA%\mochi stay, as with other apps.
Section "Uninstall"
  Delete "$SMPROGRAMS\Mochi.lnk"
  Delete "$DESKTOP\Mochi.lnk"
  Delete "$INSTDIR\mochi.exe"
  Delete "$INSTDIR\FONTS-LICENSE.txt"
  Delete "$INSTDIR\software-gl\mochi.exe"
  Delete "$INSTDIR\software-gl\opengl32.dll"
  Delete "$INSTDIR\software-gl\libgallium_wgl.dll"
  Delete "$INSTDIR\software-gl\MESA-LICENSE.txt"
  RMDir "$INSTDIR\software-gl"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINST}"
SectionEnd
