@echo off
setlocal
set "REPO=%~dp0"
set "RUST_ROOT=%~dp0..\shared\runtimes\anchorbell-rust"
set "CARGO_HOME=%RUST_ROOT%\cargo"
set "RUSTUP_HOME=%RUST_ROOT%\rustup"
set "RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc"
start "" powershell -NoProfile -WindowStyle Hidden -Command "Start-Sleep -Seconds 2; Start-Process 'http://127.0.0.1:8787'"
pushd "%REPO%"
"%RUST_ROOT%\cargo\bin\cargo.exe" run -p anchorbell-engine --bin anchorbell_dashboard --locked
popd
pause
