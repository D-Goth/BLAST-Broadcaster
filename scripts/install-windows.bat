@echo off
setlocal
cd /d "%~dp0.."

set "DEST=%LOCALAPPDATA%\Programs\BLAST Broadcaster"
set "LNK=%APPDATA%\Microsoft\Windows\Start Menu\Programs\BLAST Broadcaster.lnk"

if /i "%~1"=="/uninstall" goto uninstall

where cargo >nul 2>&1
if errorlevel 1 (
  echo Rust est introuvable. Installe-le depuis https://rustup.rs puis rouvre l'invite de commandes.
  exit /b 1
)

echo Compilation de BLAST Broadcaster (release)...
cargo build --release -p blast-app -p blast-cli
if errorlevel 1 (
  echo La compilation a echoue. Verifie que les outils C++ de Visual Studio sont installes.
  exit /b 1
)

if not exist "%DEST%" mkdir "%DEST%"
copy /y "target\release\blast-broadcaster.exe" "%DEST%\" >nul
copy /y "target\release\blast.exe" "%DEST%\" >nul
copy /y "app\src-tauri\icons\icon.ico" "%DEST%\" >nul

set "VBS=%TEMP%\blast-shortcut.vbs"
> "%VBS%" echo Set s = CreateObject("WScript.Shell").CreateShortcut(WScript.Arguments(0))
>> "%VBS%" echo s.TargetPath = WScript.Arguments(1)
>> "%VBS%" echo s.WorkingDirectory = WScript.Arguments(2)
>> "%VBS%" echo s.IconLocation = WScript.Arguments(3)
>> "%VBS%" echo s.Description = "Console de webradio et de live"
>> "%VBS%" echo s.Save
cscript //nologo "%VBS%" "%LNK%" "%DEST%\blast-broadcaster.exe" "%DEST%" "%DEST%\icon.ico"
del "%VBS%" >nul 2>&1

echo.
echo BLAST Broadcaster est installe :
echo   menu Demarrer : BLAST Broadcaster
echo   dossier       : %DEST%
echo Clic droit sur l'entree du menu Demarrer pour l'epingler a la barre des taches.
exit /b 0

:uninstall
if exist "%LNK%" del "%LNK%"
if exist "%DEST%" rmdir /s /q "%DEST%"
echo BLAST Broadcaster est desinstalle. Tes reglages restent dans %APPDATA%\fr.black-lab.blast
exit /b 0
