
#bash stm32/build_for_bootloader.sh

# build bootloader
pushd stm32_bootloader
cargo clean && cargo build --release && cargo objcopy --bin stm32_bootloader --release -- -O binary target/bootloader.bin
popd

# build app
pushd stm32/app
cargo clean && cargo +nightly build --release && cargo objcopy --bin app --release -- -O binary ../target/app.bin
popd

python build_flash.py