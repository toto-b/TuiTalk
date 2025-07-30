cargo build --target wasm32-unknown-unknown --release
wasm-bindgen ../target/wasm32-unknown-unknown/release/wasm_client.wasm --out-dir target-wasm
