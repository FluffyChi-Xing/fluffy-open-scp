@echo off
setlocal EnableExtensions EnableDelayedExpansion
title OpenSCP - archive release and clean target

rem ============================================================
rem  scripts\archive-release.bat - archive release, clean target
rem  1. Read version from src-tauri\tauri.conf.json, fall back to
rem     parsing the bundle file name.
rem  2. If the archive dir has no vVERSION subfolder yet, copy the
rem     nsis setup.exe, the msi, target\release\fluffy-open-scp.exe
rem     and a certutil-generated SHA256SUMS.txt there, following the
rem     existing vX.Y.Z convention.
rem  3. Delete the project-root target build directory.
rem  Archive dir default: D:\rust\packages\fluffy-open-scp-releases
rem  Override it with the ARCHIVE_DIR env var, e.g. for testing.
rem  NOTE: this file is ANSI/GBK encoded, not UTF-8 - keep it that
rem  way when editing, and keep cmd-special chars out of all text.
rem ============================================================

set "PROJ=%~dp0"
if not defined ARCHIVE_DIR set "ARCHIVE_DIR=D:\rust\packages\fluffy-open-scp-releases"

rem ---- 1. version ----
set "VER="
set "RAW="
for /f "tokens=2 delims=:, " %%a in ('findstr /C:"\"version\":" "%PROJ%src-tauri\tauri.conf.json" 2^>nul') do set "RAW=%%a"
if defined RAW set "VER=%RAW:"=%"
if not defined VER (
    for %%f in ("%PROJ%target\release\bundle\nsis\OpenSCP_*_x64-setup.exe") do (
        for /f "tokens=2 delims=_" %%v in ("%%~nxf") do set "VER=%%v"
    )
)
if not defined VER (
    echo [错误] 无法确定版本号: 读不到 tauri.conf.json 的 version,bundle 里也没有安装包。
    goto :abort
)
echo [INFO] 版本: %VER%
echo [INFO] 归档: %ARCHIVE_DIR%
echo.

set "VDIR=%ARCHIVE_DIR%\v%VER%"
set "SUMS=%VDIR%\SHA256SUMS.txt"

rem ---- 2. backup if this version is not archived yet ----
if exist "%VDIR%" (
    echo [SKIP] 归档已存在,跳过备份: %VDIR%
    goto :cleanup
)

set "SETUP=%PROJ%target\release\bundle\nsis\OpenSCP_%VER%_x64-setup.exe"
set "MSI=%PROJ%target\release\bundle\msi\OpenSCP_%VER%_x64_en-US.msi"
set "BIN=%PROJ%target\release\fluffy-open-scp.exe"

if not exist "%SETUP%" if not exist "%MSI%" (
    echo [WARN] target\release\bundle 里没有版本 %VER% 的安装包。
    choice /C YN /M "没有可备份的内容,仍要继续清理 target 吗"
    if errorlevel 2 goto :abort
    goto :cleanup
)

echo [INFO] 备份新版本到 %VDIR% ...
mkdir "%VDIR%" 2>nul
if exist "%SETUP%" (copy /y "%SETUP%" "%VDIR%\" >nul || echo [WARN] setup.exe 复制失败)
if exist "%MSI%"   (copy /y "%MSI%"   "%VDIR%\" >nul || echo [WARN] msi 复制失败)
if exist "%BIN%"   (copy /y "%BIN%"   "%VDIR%\" >nul || echo [WARN] 主程序 fluffy-open-scp.exe 复制失败,可能被占用)

rem write SHA256SUMS.txt in sha256sum format: hash + two spaces + name
if exist "%SUMS%" del "%SUMS%"
for %%f in ("%VDIR%\fluffy-open-scp.exe" "%VDIR%\OpenSCP_%VER%_x64-setup.exe" "%VDIR%\OpenSCP_%VER%_x64_en-US.msi") do (
    if exist "%%f" call :write_hash "%%~ff" "%%~nxf"
)
if exist "%SUMS%" (
    echo [OK] SHA256SUMS.txt:
    type "%SUMS%"
) else (
    echo [WARN] 没有任何文件被备份,未生成 SHA256SUMS.txt。
)
echo.

rem ---- 3. clean target ----
:cleanup
tasklist /FI "IMAGENAME eq cargo.exe" 2>nul | findstr /I "cargo.exe" >nul && echo [WARN] 检测到 cargo.exe 正在运行,target 可能删不掉或马上被重建。
tasklist /FI "IMAGENAME eq rustc.exe" 2>nul | findstr /I "rustc.exe" >nul && echo [WARN] 检测到 rustc.exe 正在运行,可能有构建进行中。
echo [INFO] 删除 %PROJ%target ...
rd /s /q "%PROJ%target" 2>nul
if exist "%PROJ%target" (
    echo [WARN] target 未能完全删除: 部分文件被占用,例如资源管理器停在里面,或构建进程未退出。已删除的部分保持删除。
) else (
    echo [OK] target 已清理。
)
goto :done

:write_hash
for /f "delims=" %%h in ('certutil -hashfile "%~1" SHA256 ^| findstr /R "^[0-9a-f][0-9a-f][0-9a-f][0-9a-f]"') do >>"%SUMS%" echo %%h  %~2
goto :eof

:abort
echo.
echo [中止] 保持 target 不动,本次未清理。
goto :finish

:done
echo.
echo [完成] 归档目录: %ARCHIVE_DIR%
:finish
echo.
pause
endlocal
