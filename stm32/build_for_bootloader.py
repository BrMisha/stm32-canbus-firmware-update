import os
import sys

flash_start = int(0x08000000)
flash_size = int(128 * 1024)
bootloader_size = int(5 * 1024)

device_descriptor_size = int(1024)
bootloader_data_size = int(1024)
data_size = int(1024)

device_descriptor = flash_start + bootloader_size
bootloader_data = device_descriptor_size + flash_start + 1024

app_size = int((flash_size - bootloader_size - device_descriptor_size - bootloader_data_size - (data_size * 2)) / 2)

app1 = int(flash_start + bootloader_size + bootloader_data_size)
data1 = int(app1 + app_size)

app2 = int(data1 + data_size)
data2 = int(app2 + app_size)

print("app_size", hex(app_size))
print("bootloader data", hex(bootloader_data))
print("app1", hex(app1))
print("data1", hex(data1))
print("app2", hex(app2))
print("data2", hex(data2))

def generate(app_addr):
    txt = "MEMORY\n\
    {{\n\
        FLASH : ORIGIN = 0x{:X}, LENGTH = {}K\n\
        RAM : ORIGIN = 0x20000000, LENGTH = 20K\n\
    }}\n".format(app_addr, int(app_size / 1024))
    print(txt)

    with open('memory.x', 'w') as f:
        f.write(txt)

    os.system("cd app && cargo clean && cargo +nightly build --release")

    os.remove('memory.x')

    pass


if len(sys.argv) == 2:
    out_path = os.path.abspath(sys.argv[1])
    print("Output path: ", out_path)

    os.rename('memory.x', 'memory.x_')

    try:
        objcopy = "cd app && cargo objcopy --bin app --release -- -O binary {}"
        generate(app1)
        os.system(objcopy.format(str(out_path + "/app1.bin")))

        #generate(app2)
        #os.system(objcopy.format(str(out_path + "/app2.bin")))
    except Exception as e:
        print(e)

    os.rename('memory.x_', 'memory.x')

    pass
