@echo off
setlocal
cd /d "%~dp0"

python -m pip install -r requirements.txt
if errorlevel 1 goto :failed

for /f "delims=" %%S in ('python -c "import sysconfig; print(sysconfig.get_path('scripts'))"') do set "PY_SCRIPTS=%%S"
if not exist "%PY_SCRIPTS%\pyside6-deploy.exe" goto :missing_deploy

if not exist "dist" mkdir "dist"
if errorlevel 1 goto :failed

"%PY_SCRIPTS%\pyside6-deploy.exe" -c pysidedeploy.spec --force --nuitka-version 4.3.4
if errorlevel 1 goto :failed
if not exist "dist\ProxyCraft.exe" goto :missing_exe

if exist "dist\assets" rmdir /S /Q "dist\assets"
copy /Y "README.md" "dist\README.md" >nul
if errorlevel 1 goto :failed
copy /Y "LICENSE" "dist\LICENSE.txt" >nul
if errorlevel 1 goto :failed
copy /Y "THIRD_PARTY_NOTICES.md" "dist\THIRD_PARTY_NOTICES.md" >nul
if errorlevel 1 goto :failed
if not exist "dist\licenses" mkdir "dist\licenses"
copy /Y "licenses\*.txt" "dist\licenses\" >nul
if errorlevel 1 goto :failed

rem Included FFmpeg: downloaded once into vendor\ffmpeg by get_ffmpeg.ps1.
if not exist "vendor\ffmpeg\ffmpeg.exe" (
    powershell -NoProfile -ExecutionPolicy Bypass -File "get_ffmpeg.ps1"
    if errorlevel 1 goto :failed
)
if exist "dist\ffmpeg" rmdir /S /Q "dist\ffmpeg"
xcopy /E /I /Y /Q "vendor\ffmpeg" "dist\ffmpeg" >nul
if errorlevel 1 goto :failed

rem Release ZIPs: the EXE together with the licence files Qt's LGPL and FFmpeg's GPL require.
for /f "delims=" %%V in ('python -c "import proxycraft; print(proxycraft.__version__)"') do set "APP_VERSION=%%V"
set "ZIP_NAME=ProxyCraft-%APP_VERSION%-Windows.zip"
set "ZIP_NO_FFMPEG=ProxyCraft-%APP_VERSION%-Windows-without-FFmpeg.zip"
set "COMMON='dist\ProxyCraft.exe','dist\README.md','dist\LICENSE.txt','dist\THIRD_PARTY_NOTICES.md','dist\licenses'"
powershell -NoProfile -Command "Compress-Archive -Force -Path %COMMON%,'dist\ffmpeg' -DestinationPath 'dist\%ZIP_NAME%'"
if errorlevel 1 goto :failed
powershell -NoProfile -Command "Compress-Archive -Force -Path %COMMON% -DestinationPath 'dist\%ZIP_NO_FFMPEG%'"
if errorlevel 1 goto :failed

rem Installer (needs Inno Setup 6: winget install --id JRSoftware.InnoSetup -e).
set "ISCC="
for %%I in (iscc.exe) do if not "%%~$PATH:I"=="" set "ISCC=%%~$PATH:I"
if not defined ISCC if exist "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" set "ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe"
if not defined ISCC if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not defined ISCC if exist "%ProgramFiles%\Inno Setup 6\ISCC.exe" set "ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe"
set "SETUP_NAME=ProxyCraft-%APP_VERSION%-Setup.exe"
if defined ISCC (
    "%ISCC%" /Q /DAppVersion=%APP_VERSION% "installer\ProxyCraft.iss"
    if errorlevel 1 goto :failed
) else (
    set "SETUP_NAME="
    echo Inno Setup 6 was not found, so no installer was made.
)

echo.
echo Build complete.
echo EXE: dist\ProxyCraft.exe (finds dist\ffmpeg next to it)
if defined SETUP_NAME echo Installer:          dist\%SETUP_NAME%
echo ZIP with FFmpeg:    dist\%ZIP_NAME%
echo ZIP without FFmpeg: dist\%ZIP_NO_FFMPEG%
echo FFmpeg source to attach to the release: vendor\release-sources\
pause
exit /b 0

:missing_deploy
echo Could not find pyside6-deploy.exe in the Python Scripts folder.
echo Check that Python was installed correctly, then run build.bat again.
goto :failed

:missing_exe
echo The build finished without creating dist\ProxyCraft.exe.
goto :failed

:failed
echo.
echo Build failed. Read the error above, fix the indicated issue, and retry.
pause
exit /b 1
