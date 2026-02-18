@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Insiders\Common7\Tools\VsDevCmd.bat" -no_logo -arch=amd64
set PATH=C:\Users\sam.mckoy\.cargo\bin;%PATH%
echo === VsDevCmd loaded (amd64) ===
where link.exe
echo === Starting cargo build ===
cargo build --release -p but-server 2>&1
echo === Build exit code: %ERRORLEVEL% ===
