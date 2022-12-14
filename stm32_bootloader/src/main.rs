#![allow(clippy::empty_loop)]
//#![deny(unsafe_code)]
#![no_main]
#![no_std]

use core::ptr::write;
use panic_semihosting as _;

//use cortex_m_semihosting::hprintln;
use stm32f1xx_hal as _;

//use cortex_m::Peripherals;
//use crate::_::pac::Peripherals;
use stm32f1xx_hal::time;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::flash::FlashExt;
use stm32f1xx_hal::prelude::_stm32_hal_rcc_RccExt;
use stm32f1xx_hal::gpio::GpioExt;
use stm32f1xx_hal::afio::AfioExt;
use stm32f1xx_hal::{
    can::Can,
    gpio::{
        gpioa::{PA2, PA3},
        Output, PushPull,
    },
    pac::CAN1,
    prelude::*,
    serial::{Config, Serial},
};
use cortex_m_rt::entry;
use core::fmt::Write;

const PAGE_SIZE: u32 = 1024;
const FW_BEGIN: u32 = 20 * 1024;
const NEW_FW_BEGIN: u32 = FW_BEGIN + (53 * 1024);

#[entry]
fn main() -> ! {
    let dev_p = pac::Peripherals::take().unwrap();
    let mut flash = dev_p.FLASH.constrain();
    let rcc = dev_p.RCC.constrain();

    let clocks = rcc
        .cfgr
        .freeze(&mut flash.acr);

    let mut afio = dev_p.AFIO.constrain();
    let mut gpioa = dev_p.GPIOA.split();

    let tx = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);
    let rx = gpioa.pa10;

    let mut serial = Serial::new(
        dev_p.USART1,
        (tx, rx),
        &mut afio.mapr,
        Config::default().baudrate(115200_u32.bps()),
        &clocks,
    );

    serial.bwrite_all(b"...Bootloader stated...\r\n");

    let pf = helpers::pending_fw::get(NEW_FW_BEGIN);
    match pf {
        Some(v) => {
            write!(serial, "Updating firmware to {}.{}.{} {}\r\n", v.0.major, v.0.minor, v.0.path, v.0.build);

            let mut w = flash.writer(stm32f1xx_hal::flash::SectorSize::Sz1K, stm32f1xx_hal::flash::FlashSize::Sz128K);

            for (page_n, data) in v.1.chunks(1024).enumerate() {
                let page_p = FW_BEGIN + (PAGE_SIZE * page_n as u32);
                if let Err(e) = w.page_erase(page_p) {
                    write!(serial, "Erase error {:?}\r\n", e).unwrap();
                }

                if let Err(e) = w.write(page_p, data) {
                    write!(serial, "Write error {:?}\r\n", e).unwrap();
                }
            }

            let flash_data = unsafe {
                core::slice::from_raw_parts(&*(FW_BEGIN as *const u8), v.1.len())
            };

            for (pos, (v1, v2)) in flash_data.iter().zip(v.1).enumerate() {
                if v1 != v2 {
                    write!(serial, "Error from {:?}\r\n", pos).unwrap();
                }
            }

            // erase
            w.page_erase(NEW_FW_BEGIN);
        },
        None => {},
    };

    serial.bwrite_all(b"Jump\r\n");
    jump_to_main(0x8000000 + FW_BEGIN);

    fn jump_to_main(address: u32) {
        unsafe {
            #[allow(unused_mut)]
                let mut p = cortex_m::Peripherals::steal();
            #[cfg(not(armv6m))]
            p.SCB.invalidate_icache();
            p.SCB.vtor.write(address as u32);
            cortex_m::asm::bootload(address as *const u32);
        }
    }

    loop {}
}