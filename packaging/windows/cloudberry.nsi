; NSIS installer: makensis /DVERSION=0.1.0 packaging/windows/cloudberry.nsi  (run from the repo root)
!define APPNAME "Cloudberry"
Name "${APPNAME}"
OutFile "dist\Cloudberry-windows-x86_64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\${APPNAME}"
RequestExecutionLevel user
Icon "assets\icons\app\cloudberry.ico"
UninstallIcon "assets\icons\app\cloudberry.ico"
Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles
Section
  SetOutPath "$INSTDIR"
  File "target\release\cloudberry.exe"
  File "assets\icons\app\cloudberry.ico"
  File "LICENSE"
  CreateShortcut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\cloudberry.exe" "" "$INSTDIR\cloudberry.ico"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayVersion" "${VERSION}"
SectionEnd
Section "Uninstall"
  Delete "$INSTDIR\cloudberry.exe"
  Delete "$INSTDIR\cloudberry.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"
SectionEnd
