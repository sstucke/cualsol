@echo off
setlocal
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where gcc >nul 2>&1
if errorlevel 1 (
  for /d %%D in ("%LOCALAPPDATA%\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_*") do (
    if exist "%%D\mingw64\bin\gcc.exe" set "PATH=%%D\mingw64\bin;%PATH%"
  )
)
set "APP=%~dp0target\debug\cualsol.exe"
if not exist "%APP%" (
  echo Compilando Cual Solitaire...
  cargo build -p cualsol
  if errorlevel 1 (
    echo.
    echo No se pudo compilar.
    pause
    exit /b 1
  )
)
start "" "%APP%"
