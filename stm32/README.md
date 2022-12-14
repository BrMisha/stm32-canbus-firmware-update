to build file:

cd app
cargo clean
cargo objcopy --bin app --release -- -O binary ../target/app.bin

to add headers:
cd ..
rust-script add_header.rs target/app.bin