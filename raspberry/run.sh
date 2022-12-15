echo ${RASP} &&\
cross build --target aarch64-unknown-linux-gnu &&\
rsync target/aarch64-unknown-linux-gnu/debug/canbus_raspberry pi@${RASP}:~/ &&\
ssh pi@${RASP} ./canbus_raspberry show-serials