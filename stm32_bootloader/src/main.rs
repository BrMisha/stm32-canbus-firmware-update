#![allow(clippy::empty_loop)]
//#![deny(unsafe_code)]
#![no_main]
#![no_std]

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

    let mut serial = Serial::usart1(
        dev_p.USART1,
        (tx, rx),
        &mut afio.mapr,
        Config::default().baudrate(115200_u32.bps()),
        clocks,
    );

    serial.bwrite_all(b"...Bootloader stated...\r\n");
    serial.bwrite_all(b"Jump\r\n");
    jump_to_main(0x8002000);

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