to build file:

cd app
cargo clean
cargo objcopy --bin app --release -- -O binary ../target/app.bin

to add headers:
cd ..
rust-script add_header.rs target/app.bin

to flash:
st-flash write target/app.bin 0x08005000

or flash for reflash with bootloader
st-flash write target/app.bin 0x08012400