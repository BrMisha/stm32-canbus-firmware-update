cd "$(dirname "$0")"
cargo clean && cargo build --release && cargo objcopy --bin stm32_bootloader --release -- -O binary target/bootloader.bin
