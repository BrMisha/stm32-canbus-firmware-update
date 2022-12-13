# build app
bash stm32/build_for_bootloader.sh

#build bootloader
pushd stm32_bootloader
cargo build --release && cargo objcopy --bin stm32_bootloader --release -- -O binary target/bootloader.bin
popd

python build_flash.py