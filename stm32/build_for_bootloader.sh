cd "$(dirname "$0")"

mkdir out

mv memory.x memory.x_

python -c "import memoryx_wizard; memoryx_wizard.create_memoryx(1)"
pushd app && cargo clean && cargo +nightly build --release && cargo objcopy --bin app --release -- -O binary ../out/app1.bin && popd
rust-script add_header.rs out/app1.bin

#python -c "import memoryx_wizard; memoryx_wizard.create_memoryx(2)"
#pushd app && cargo clean && cargo +nightly build --release && cargo objcopy --bin app --release -- -O binary ../out/app2.bin && popd
#rust-script add_header.rs out/app2.bin

mv memory.x_ memory.x