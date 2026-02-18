@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Insiders\Common7\Tools\VsDevCmd.bat" -no_logo
set PATH=C:\Users\sam.mckoy\.cargo\bin;%PATH%
echo === VsDevCmd loaded ===
where link.exe
where cargo.exe
echo === Starting cargo build ===
cargo build --release -p but-server
echo === Build exit code: %ERRORLEVEL% ===
