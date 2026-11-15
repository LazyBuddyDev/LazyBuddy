; -*- coding: utf-8 -*-
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
Name "LazyBuddy"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\LazyBuddy"
RequestExecutionLevel user
SetCompressor /SOLID zlib
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=2052 "ProductName" "LazyBuddy"
VIAddVersionKey /LANG=2052 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=2052 "FileDescription" "LazyBuddy 安装程序"
VIAddVersionKey /LANG=2052 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=2052 "LegalCopyright" "Copyright (c) 2026 Leager"
!define MUI_ICON "..\assets\lazybuddy.ico"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\lazybuddy.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_LANGUAGE "SimpChinese"

!ifndef REGKEY
!define REGKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\LazyBuddy"
!endif
Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "LazyBuddy 需要 Windows x64。" /SD IDOK
    SetErrorLevel 1
    Quit
  ${EndIf}
  SetRegView 64
FunctionEnd

Section "LazyBuddy"
  ; Never overwrite an existing installation, running image or unrelated directory.
  IfFileExists "$INSTDIR" 0 fresh
    MessageBox MB_OK|MB_ICONSTOP "请选择尚不存在的新安装目录。已有安装请使用应用内更新，或先卸载。" /SD IDOK
    SetErrorLevel 1
    Quit
  fresh:
  ReadRegStr $0 HKCU "${REGKEY}" "InstallLocation"
  StrCmp $0 "" register_ready
    MessageBox MB_OK|MB_ICONSTOP "当前用户已有安装记录，请先卸载该安装。" /SD IDOK
    SetErrorLevel 1
    Quit
  register_ready:
  ClearErrors
  SetOutPath "$INSTDIR"
  SetOverwrite off
  !include "${PAYLOAD_FILES}"
  WriteRegStr HKCU "${REGKEY}" "DisplayName" "LazyBuddy"
  WriteRegStr HKCU "${REGKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${REGKEY}" "Publisher" "Leager"
  WriteRegStr HKCU "${REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${REGKEY}" "DisplayIcon" "$INSTDIR\lazybuddy.exe"
  WriteRegStr HKCU "${REGKEY}" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegStr HKCU "${REGKEY}" "QuietUninstallString" '$\"$INSTDIR\uninstall.exe$\" --yes'
  WriteRegDWORD HKCU "${REGKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${REGKEY}" "NoRepair" 1
  IfErrors failed done
  failed:
    MessageBox MB_OK|MB_ICONSTOP "安装未完成，请检查目录和注册表权限。已写入文件可通过目录内 uninstall.exe 清理。" /SD IDOK
    SetErrorLevel 1
    Abort
  done:
SectionEnd
