@echo off
setlocal
cd /d "%~dp0"

rem ProxyCraft Windows build: EXE, ZIPs and installer in dist\.
rem Needs Rust (https://rustup.rs, MSVC toolchain) and an internet connection for the first build.

where cargo >nul 2>nul
if errorlevel 1 goto :missing_cargo

cargo test --release
if errorlevel 1 goto :failed
cargo build --release
if errorlevel 1 goto :failed
if not exist "target\release\ProxyCraft.exe" goto :missing_exe

if not exist "dist" mkdir "dist"
copy /Y "target\release\ProxyCraft.exe" "dist\ProxyCraft.exe" >nul
if errorlevel 1 goto :failed
copy /Y "README.md" "dist\README.md" >nul
if errorlevel 1 goto :failed
copy /Y "LICENSE" "dist\LICENSE.txt" >nul
if errorlevel 1 goto :failed
copy /Y "THIRD_PARTY_NOTICES.md" "dist\THIRD_PARTY_NOTICES.md" >nul
if errorlevel 1 goto :failed
if exist "dist\licenses" rmdir /S /Q "dist\licenses"
mkdir "dist\licenses"
copy /Y "licenses\*.*" "dist\licenses\" >nul
if errorlevel 1 goto :failed

rem Included FFmpeg: downloaded once into vendor\ffmpeg by get_ffmpeg.ps1.
if not exist "vendor\ffmpeg\ffmpeg.exe" (
    powershell -NoProfile -ExecutionPolicy Bypass -File "get_ffmpeg.ps1"
    if errorlevel 1 goto :failed
)
if exist "dist\ffmpeg" rmdir /S /Q "dist\ffmpeg"
xcopy /E /I /Y /Q "vendor\ffmpeg" "dist\ffmpeg" >nul
if errorlevel 1 goto :failed

rem Version from Cargo.toml (the first line that starts with version =).
for /f "tokens=2 delims==" %%V in ('findstr /B /C:"version =" Cargo.toml') do if not defined APP_VERSION set "APP_VERSION=%%~V"
set "APP_VERSION=%APP_VERSION: =%"
set "APP_VERSION=%APP_VERSION:"=%"

rem Release ZIPs: the EXE together with the licence files FFmpeg's GPL and the Rust libraries require.
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
echo Build complete: ProxyCraft %APP_VERSION%
echo EXE: dist\ProxyCraft.exe (finds dist\ffmpeg next to it)
if defined SETUP_NAME echo Installer:          dist\%SETUP_NAME%
echo ZIP with FFmpeg:    dist\%ZIP_NAME%
echo ZIP without FFmpeg: dist\%ZIP_NO_FFMPEG%
echo FFmpeg source to attach to the release: vendor\release-sources\
pause
exit /b 0

:missing_cargo
echo Rust was not found. Install it from https://rustup.rs (choose the default MSVC toolchain),
echo open a new Command Prompt, then run build.bat again.
goto :failed

:missing_exe
echo The build finished without creating target\release\ProxyCraft.exe.
goto :failed

:failed
echo.
echo Build failed. Read the error above, fix the indicated issue, and retry.
pause
exit /b 1
