//! `cargo` "language".
//!
//! ```cargo
//! [dependencies]
//! time = "0.1.25"
//! clap = "4.0.29"
//! ```

use clap::{arg, Parser};

const flash_start: u32 = 0x08000000;
const flash_size: u32 = 128 * 1024;

const bootloader_size: u32 = 5 * 1024;
const device_descriptor_size: u32 = 1024;
const bootloader_data_size: u32 = 1024;
const app_header_size: u32 = 4+8;   // len + version
const data_size: u32 = 1024;

const device_descriptor: u32 = flash_start + bootloader_size;
const bootloader_data: u32 = device_descriptor + device_descriptor_size;

const app_size: u32 = (flash_size - bootloader_size - device_descriptor_size - bootloader_data_size - ((data_size + app_header_size) * 2)) / 2;

const app_header1: u32 = bootloader_data + bootloader_data_size;
const app1: u32 = app_header1 + app_header_size;
const data1: u32 = app1 + app_size;

const app_header2: u32 = data1 + data_size;
const app2: u32 = app_header2 + app_header_size;
const data2: u32 = app2 + app_size;

#[derive(Parser)]
enum Args {
    PrintAddresses,
}

fn main() {
    let args = Args::parse();
    println!("{:?}", args);
}

fn print_addresses() {
    println!("app_size {:#x}", app_size);
    println!("device descriptor {:#x}", device_descriptor);
    println!("bootloader data {:#x}", bootloader_data);
    println!("app1 {:#x}", app1);
    println!("data1 {:#x}", data1);
    println!("app2 {:#x}", app2);
    println!("data2 {:#x}", data2);
}
