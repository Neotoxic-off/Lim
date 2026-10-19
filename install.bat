@echo off
rem lim installer for Windows: builds lim from source with cargo and puts it in %USERPROFILE%\.cargo\bin.
rem   curl -fsSLo %TEMP%\lim-install.bat https://raw.githubusercontent.com/Neotoxic-off/Lim/master/install.bat && %TEMP%\lim-install.bat
setlocal

rem LIM_REPO installs from a fork or a local clone instead.
set "repo=https://github.com/Neotoxic-off/Lim"
if defined LIM_REPO set "repo=%LIM_REPO%"

where cargo >nul 2>nul
if errorlevel 1 (
    echo lim is built from source and needs Rust 1.88+ ^(cargo^) with the MSVC build tools.
    echo Install it from https://rustup.rs, open a new terminal, then run this again.
    exit /b 1
)

rem The latest release, so lim knows its version; master when there is none (or for LIM_REPO).
set "tag="
if not defined LIM_REPO for /f "delims=" %%t in ('powershell -NoProfile -Command "try { (Invoke-RestMethod https://api.github.com/repos/Neotoxic-off/Lim/releases/latest).tag_name } catch {}"') do set "tag=%%t"

if defined tag (
    echo Building lim %tag% from %repo% ^(about a minute^)...
    cargo install --git "%repo%" --tag "%tag%" --locked --force lim
) else (
    echo Building lim master from %repo% ^(about a minute^)...
    cargo install --git "%repo%" --locked --force lim
)
if errorlevel 1 exit /b 1

set "bin=%USERPROFILE%\.cargo\bin"
if defined CARGO_HOME set "bin=%CARGO_HOME%\bin"
if defined CARGO_INSTALL_ROOT set "bin=%CARGO_INSTALL_ROOT%\bin"
echo lim installed: %bin%\lim.exe
echo Open a new terminal and run: lim
endlocal
