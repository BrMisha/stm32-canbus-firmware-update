import sys
import json

flash_start = int(0x08000000)
flash_size = int(128 * 1024)

'''
bootloader (5K)
device_descriptor (1K)
bootloader_data (1K)
app_header1
app1
data1 (1K)
app_header2
app2
data2 (1K)
'''

bootloader_size = int(5 * 1024)
device_descriptor_size = int(1024)
bootloader_data_size = int(1024)
app_header_size = int(4)   # len + version
data_size = int(1024)

device_descriptor = flash_start + bootloader_size
bootloader_data = device_descriptor + device_descriptor_size

app_size = int((flash_size - bootloader_size - device_descriptor_size - bootloader_data_size - (data_size * 2)) / 2)

app1 = int(bootloader_data + bootloader_data_size)
data1 = int(app1 + app_size)

app2 = int(data1 + data_size)
data2 = int(app2 + app_size)

def print_addresses():
    print("app_header_size", hex(app_header_size))
    print("app_size", hex(app_size))
    print("device descriptor", hex(device_descriptor))
    print("bootloader data", hex(bootloader_data))
    print("app1", hex(app1))
    print("data1", hex(data1))
    print("app2", hex(app2))
    print("data2", hex(data2))
    pass


def get_addresses():
    return (flash_start, device_descriptor, bootloader_data, app1)

def generate(app_addr):
    txt = "MEMORY\n\
    {{\n\
        FLASH : ORIGIN = 0x{:X}, LENGTH = {}K\n\
        RAM : ORIGIN = 0x20000000, LENGTH = 20K\n\
    }}\n".format(app_addr+app_header_size, int(app_size / 1024))
    print(txt)

    f = open('memory.x', 'w')
    f.write(txt)
    f.close()

    pass

def create_memoryx(app):
    print("App: ", app)

    if app == 1:
        generate(app1)
    elif app == 2:
        generate(app2)
    else:
        print("wrong app number")

    pass
