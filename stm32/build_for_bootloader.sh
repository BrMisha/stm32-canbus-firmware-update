mkdir out

mv memory.x memory.x_

python memoryx_wizard.py 1
pushd app && cargo clean && cargo +nightly build --release && cargo objcopy --bin app --release -- -O binary ../out/app1.bin && popd

python memoryx_wizard.py 2
pushd app && cargo clean && cargo +nightly build --release && cargo objcopy --bin app --release -- -O binary ../out/app2.bin && popd

mv memory.x_ memory.x