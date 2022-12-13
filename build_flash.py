import stm32.memoryx_wizard as wizard



flash_start, p_device_descriptor, p_bootloader_data, p_app1 = wizard.get_addresses()
p_device_descriptor = p_device_descriptor - flash_start
p_bootloader_data = p_bootloader_data - flash_start
p_app1 = p_app1 - flash_start

print("p_device_descriptor", p_device_descriptor)
print("p_bootloader_data", p_bootloader_data)
print("p_app1", p_app1)

f = open('stm32_bootloader/target/bootloader.bin', 'rb')
array = bytearray(f.read())
f.close()

# fill up to p_app1
s = len(array)
for x in range(s, p_app1):
    array.append(0xFF)

# active app1
array[p_bootloader_data] = 1

f = open('stm32/out/app1.bin', 'rb')
array.extend(f.read())
f.close()

f = open('flash.bin', 'wb')
f.write(array)
f.close()
