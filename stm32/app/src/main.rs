//#![deny(warnings)]
#![no_main]
#![cfg_attr(not(test), no_std)]

mod util;

use core::fmt::Write;
//use cortex_m_semihosting::hprintln;

use num_traits::cast::ToPrimitive;
use panic_halt as _;
use rtic::app;
use stm32f1xx_hal::pac::Interrupt;
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
use systick_monotonic::Systick;

pub const DEVICE_SERIAL: canbus_common::frames::serial::Serial =
    canbus_common::frames::serial::Serial([1, 2, 3, 4, 5]);
pub const PAGE_SIZE: usize = 1024;
pub const NEW_FW_BEGIN: usize = (20 + 53) * 1024;

#[app(device = stm32f1xx_hal::pac, peripherals = true, dispatchers = [SPI1, SPI2])]
mod app {
    use super::*;
    use bxcan::Fifo;
    use canbus_common::frame_id::SubId;
    use canbus_common::frames::Type;
    use helpers::firmware_update::PutPartError;
    use stm32f1xx_hal::gpio;
    use stm32f1xx_hal::gpio::Floating;
    use stm32f1xx_hal::pac::USART1;

    #[derive(Default)]
    pub struct FwUpload {
        pub data: helpers::firmware_update::FirmwareUpdate<PAGE_SIZE, 5, { PAGE_SIZE + 5 }>,
        pub paused: bool,
        pub finished: bool,
        pub has_pending_fw: bool,
    }

    #[shared]
    struct Shared {
        led: PA2<Output<PushPull>>,
        led2: PA3<Output<PushPull>>,
        serial: Serial<
            USART1,
            (
                gpio::PA9<gpio::Alternate<PushPull>>,
                gpio::PA10<gpio::Input<Floating>>,
            ),
        >,

        dyn_id: canbus_common::frame_id::SubId,

        can_tx_queue: heapless::binary_heap::BinaryHeap<util::can::PriorityFrame, heapless::binary_heap::Max, 16>,
        tx_count: usize,

        fw_upload: FwUpload,
        pending_fw_version_required: bool,
    }

    #[local]
    struct Local {
        can_tx: bxcan::Tx<Can<CAN1>>,
        can_rx: bxcan::Rx0<Can<CAN1>>,

        flash: stm32f1xx_hal::flash::Parts,
        //flash_writer: stm32f1xx_hal::flash::FlashWriter<'static>,
    }

    #[monotonic(binds = SysTick, default = true)]
    type MonoTimer = Systick<1000>;

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        // Setup clocks
        let mut flash = ctx.device.FLASH.constrain();
        let rcc = ctx.device.RCC.constrain();

        // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
        // `clocks`
        //let clocks = rcc.cfgr.freeze(&mut flash.acr);

        //rtt_init_print!();
        //rprintln!("ffffffffff");

        let clocks = rcc
            .cfgr
            .use_hse(8.MHz())
            .sysclk(64.MHz())
            .hclk(64.MHz())
            .pclk1(8.MHz())
            .pclk2(64.MHz())
            .freeze(&mut flash.acr);

        let mut afio = ctx.device.AFIO.constrain();

        let mono = Systick::new(ctx.core.SYST, 8_000_000);
        //let mut delay = delay::Delay::new(ctx.core.SYST, 36_000_000);

        let mut gpioa = ctx.device.GPIOA.split();
        let led = gpioa.pa2.into_push_pull_output(&mut gpioa.crl);
        let led2 = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);

        // USART1
        let tx = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);
        let rx = gpioa.pa10;

        let mut serial = Serial::new(
            ctx.device.USART1,
            (tx, rx),
            &mut afio.mapr,
            Config::default().baudrate(115200.bps()),
            &clocks,
        );
        write!(serial, "stadte: {:?}\r\n", 1).unwrap();
        // Schedule the blinking task
        //blink::spawn_after(Duration::<u64, 1, 1000>::from_ticks(1000)).unwrap();
        //pollbq::spawn_after(Duration::<u64, 1, 1000>::from_ticks(1000)).unwrap();

        let can = Can::new(ctx.device.CAN1, ctx.device.USB);

        let mut gpiob = ctx.device.GPIOB.split();

        // Select pins for CAN1.
        let can_rx_pin = gpiob.pb8.into_floating_input(&mut gpiob.crh);
        let can_tx_pin = gpiob.pb9.into_alternate_push_pull(&mut gpiob.crh);
        can.assign_pins((can_tx_pin, can_rx_pin), &mut afio.mapr);

        // APB1 (PCLK1): 16MHz, Bit rate: 1000kBit/s, Sample Point 87.5%
        // Value was calculated with http://www.bittiming.can-wiki.info/
        let mut can = bxcan::Can::builder(can)
            //.set_bit_timing(0x001c_0000)
            .set_bit_timing(0x001c0003)
            .leave_disabled();

        can.modify_filters()
            .enable_bank(0, Fifo::Fifo0, bxcan::filter::Mask32::accept_all());
        /*.enable_bank(
            0,
            Mask32::frames_with_ext_id(
                ExtendedId::new(2).unwrap(),
                ExtendedId::new(0xFF).unwrap(),
            ),
        );*/

        // Sync to the bus and start normal operation.
        can.enable_interrupts(
            bxcan::Interrupts::TRANSMIT_MAILBOX_EMPTY | bxcan::Interrupts::FIFO0_MESSAGE_PENDING,
        );
        nb::block!(can.enable_non_blocking()).unwrap();

        let (can_tx, can_rx, _) = can.split();

        let can_tx_queue = heapless::binary_heap::BinaryHeap::new();

        //rtic::pend(Interrupt::USB_HP_CAN_TX);
        //let _tt: Option<euc_can_common::Message> = None;

        (
            Shared {
                led,
                led2,
                serial,
                dyn_id: canbus_common::frame_id::SubId(0),
                can_tx_queue,
                tx_count: 0,
                fw_upload: Default::default(),
                pending_fw_version_required: false,
            },
            Local {
                can_tx,
                can_rx,
                flash,
            },
            init::Monotonics(mono),
        )
    }

    #[idle(shared = [can_tx_queue, fw_upload, pending_fw_version_required, serial], local = [flash])]
    fn idle(mut cx: idle::Context) -> ! {
        cx.shared.can_tx_queue.lock(|can_tx_queue| {
            util::can::enqueue_frame(
                can_tx_queue,
                util::can::PriorityFrame(canbus_common::frames::Frame::Serial(Type::Data(
                    DEVICE_SERIAL,
                ))),
            );
        });

        loop {
            cx.shared.fw_upload.lock(|fw_upload: &mut FwUpload| {
                if let Some(page) = fw_upload.data.get_page() {
                    let page_p = (NEW_FW_BEGIN + (PAGE_SIZE * page.1)) as u32;
                    //hprintln!("page {:?} {:?}", page.1, page_p);

                    let mut writer: stm32f1xx_hal::flash::FlashWriter = cx.local.flash.writer(
                        stm32f1xx_hal::flash::SectorSize::Sz1K,
                        stm32f1xx_hal::flash::FlashSize::Sz128K,
                    );
                    //writer.change_verification(false);
                    //let r = writer.erase(page_p, PAGE_SIZE);
                    if let Err(e) = writer.page_erase(page_p) {
                        cx.shared.serial.lock(|serial| {
                            write!(serial, "erase {:?}\r\n", e).unwrap();
                        });
                    }

                    if let Err(e) = writer.write(page_p, page.0) {
                        cx.shared.serial.lock(|serial| {
                            write!(serial, "write {:?}\r\n", e).unwrap();
                        });
                    }

                    //hprintln!("read {:?} ", writer.read(page_p, 4).unwrap());

                    fw_upload.data.remove_page();
                    //hprintln!("removed_page {}", fw_upload.data.len());

                    fw_upload.has_pending_fw = false;
                }

                if fw_upload.finished {
                    fw_upload.finished = false;
                    fw_upload.data.reset();
                    //hprintln!("finished");
                    cx.shared.serial.lock(|serial| {
                        write!(serial, "Finished\r\n").unwrap();
                    });
                }

                if fw_upload.paused {
                    fw_upload.paused = false;

                    cx.shared.can_tx_queue.lock(|can_tx_queue| {
                        util::can::enqueue_frame(
                            can_tx_queue,
                            util::can::PriorityFrame(canbus_common::frames::Frame::FirmwareUploadPause(
                                fw_upload.paused,
                            )),
                        );
                    });
                }
            });

            if cx
                .shared
                .pending_fw_version_required
                .lock(|pending_fw_version_required| *pending_fw_version_required)
            {
                let begin = NEW_FW_BEGIN as u32;

                let pf = helpers::pending_fw::get(begin);

                cx.shared.fw_upload.lock(|fw_upload: &mut FwUpload| {
                    fw_upload.has_pending_fw = pf.is_some();
                });

                cx.shared.can_tx_queue.lock(|can_tx_queue| {
                    util::can::enqueue_frame(
                        can_tx_queue,
                        util::can::PriorityFrame(canbus_common::frames::Frame::PendingFirmwareVersion(
                            Type::Data(pf.map(|v| v.0)),
                        )),
                    );
                });

                //hprintln!("crc {} {}", crc, hasher.finalize());

                cx.shared
                    .pending_fw_version_required
                    .lock(|pending_fw_version_required| {
                        *pending_fw_version_required = false;
                    })
            }
        }
    }

    use crate::util::can::can_tx;
    extern "Rust" {
        #[task(binds = USB_HP_CAN_TX, local = [can_tx], shared = [can_tx_queue, tx_count, led2, dyn_id, serial])]
        fn can_tx(mut cx: can_tx::Context);
    }

    use crate::util::can::can_rx0;
    extern "Rust" {
        #[task(binds = USB_LP_CAN_RX0, local = [can_rx], shared = [can_tx_queue, led2, dyn_id, fw_upload, pending_fw_version_required, serial])]
        fn can_rx0(mut cx: can_rx0::Context);
    }
}
