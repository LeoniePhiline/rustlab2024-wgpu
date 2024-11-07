# RustLab 2024 wgpu workshop

Chapters:

1. Basics of wgpu
2. Tracing rays (with compute shaders)
3. Bringing it to the web (with WebAssembly)

Checkout the respective branch for the chapter you are currently working on (ch1, ch2 or ch3).

## Chapter 3

Tasks:

1. Install the wasm32-unknown-unknown target for your Rust toolchain
   ```bash
   rustup target add wasm32-unknown-unknown
   ```
2. Install wasm-bindgen-cli 0.2.95
   ```bash
   cargo install -f wasm-bindgen-cli --version 0.2.95
   ```
3. Build the WASM binary
   ```bash
   RUSTFLAGS=--cfg=web_sys_unstable_apis \
     cargo build --target wasm32-unknown-unknown --release
   ```
4. Run wasm-bindgen to generate the boilerplate
   ```bash
   wasm-bindgen --out-dir public \
     --web target/wasm32-unknown-unknown/release/rustlab2024-wgpu.wasm
   ```
5. Run a local web server on the public/ directory
   ```bash
   cargo install simple-http-server
   simple-http-server ./public
   # => Running on http://localhost:8000
   ```
6. Try out your application in Chrome, Firefox Nightly and/or Safari TP
