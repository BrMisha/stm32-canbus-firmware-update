f = open('stm32_bootloader/target/bootloader.bin', 'rb')
array = bytearray(f.read())
f.close()

# fill up to app
s = len(array)
for x in range(s, 0x5000):
    array.append(0xFF)

f = open('stm32/target/app.bin', 'rb')
array.extend(f.read())
f.close()

f = open('flash.bin', 'wb')
f.write(array)
f.close()
