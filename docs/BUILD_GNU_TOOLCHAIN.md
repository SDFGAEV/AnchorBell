# GNU toolchain build

The Windows build uses the portable runtime at:

E:\Agent-Research-Workspace\shared\runtimes\anchorbell-rust

The runtime is configured with the stable x86_64-pc-windows-gnu toolchain and
the rustfmt component. Build commands must set RUSTUP_HOME, CARGO_HOME,
and PATH explicitly so the build is reproducible and does not silently fall
back to the unavailable MSVC linker.

~~~powershell
$rt = 'E:\Agent-Research-Workspace\shared\runtimes\anchorbell-rust'
$env:RUSTUP_HOME = "$rt\rustup"
$env:CARGO_HOME = "$rt\cargo"
$env:PATH = "$rt\cargo\bin;C:\Users\25676\scoop\apps\mingw-winlibs-llvm-ucrt\current\bin;$env:PATH"
Set-Location 'E:\Agent-Research-Workspace\AnchorBell'
cargo check --workspace --locked --target-dir target-gnu
cargo test --workspace --locked --target-dir target-gnu
rustfmt --edition 2021 --check engine\src\evidence.rs engine\src\validation_methods.rs
~~~

target-gnu is disposable build output and must not be treated as validation
evidence. Validation evidence is written by the simulation lab under its run directory;
the immutable core, simulator, strategy, logs, and metrics remain separate.
