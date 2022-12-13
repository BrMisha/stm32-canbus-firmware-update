//#![deny(warnings)]
#![no_main]
#![cfg_attr(not(test), no_std)]

use core::cmp::Ordering;
use core::fmt::Write;
//use cortex_m_semihosting::hprintln;
use heapless::binary_heap::{BinaryHeap, Max};
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

use num_traits::cast::ToPrimitive;

const DEVICE_SERIAL: canbus_common::frames::serial::Serial =
    canbus_common::frames::serial::Serial([1, 2, 3, 4, 5]);
const PAGE_SIZE: usize = 1024;
const NEW_FW_BEGIN: usize = (8 + 60) * 1024;

#[derive(Debug)]
pub struct PriorityFrame(pub canbus_common::frames::Frame);

impl PriorityFrame {
    pub fn to_bx_frame(&self, sub_id: canbus_common::frame_id::SubId) -> bxcan::Frame {
        let raw = self.0.raw_frame();
        let raw_id = raw.0.as_raw(sub_id);

        match raw.1 {
            canbus_common::frames::RawType::Data(v) => bxcan::Frame::new_data(
                bxcan::ExtendedId::new(raw_id).unwrap(),
                bxcan::Data::new(&v).unwrap(),
            ),
            canbus_common::frames::RawType::Remote(len) => {
                bxcan::Frame::new_remote(bxcan::ExtendedId::new(raw_id).unwrap(), len)
            }
        }
    }

    pub fn from_bxcan_frame(f: &bxcan::Frame) -> Result<Self, ()> {
        let id = match f.id() {
            bxcan::Id::Extended(id) => {
                canbus_common::frame_id::FrameId::try_from_u32_with_sub_id(id.as_raw()).ok_or(())
            }
            _ => Err(()),
        }?;

        let res = canbus_common::frames::Frame::parse_frame(
            id.0,
            match f.data() {
                Some(data) => canbus_common::frames::ParserType::Data(data),
                None => canbus_common::frames::ParserType::Remote(f.dlc()),
            },
        )
        .map_err(|_e| ())?;

        Ok(PriorityFrame(res))
    }
}

/// Ordering is based on the Identifier and frame type (data vs. remote) and can be used to sort
/// frames by priority.
impl Ord for PriorityFrame {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.0.id().to_u16() > other.0.id().to_u16() {
            return Ordering::Greater;
        } else if self.0.id().to_u16() < other.0.id().to_u16() {
            return Ordering::Less;
        }
        Ordering::Equal
    }
}

impl PartialOrd for PriorityFrame {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PriorityFrame {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for PriorityFrame {}

fn enqueue_frame(queue: &mut BinaryHeap<PriorityFrame, Max, 16>, frame: PriorityFrame) {
    if let Err(e) = queue.push(frame) {
        //hprintln!("push err {:?}", e);
        return;
    }
    rtic::pend(Interrupt::USB_HP_CAN_TX);
}

#[app(device = stm32f1xx_hal::pac, peripherals = true, dispatchers = [SPI1, SPI2])]
mod app {
    use super::*;
    use canbus_common::frame_id::SubId;
    use canbus_common::frames::Type;
    use helpers::firmware_update::PutPartError;
    use stm32f1xx_hal::gpio;
    use stm32f1xx_hal::gpio::Floating;
    use stm32f1xx_hal::pac::USART1;

    #[derive(Default)]
    pub struct FwUpload {
        data: helpers::firmware_update::FirmwareUpdate<PAGE_SIZE, 5, { PAGE_SIZE + 5 }>,
        paused: bool,
        finished: bool,
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

        can_tx_queue: BinaryHeap<PriorityFrame, Max, 16>,
        tx_count: usize,

        fw_upload: FwUpload,
        pending_fw_version_required: bool,
    }

    #[local]
    struct Local {
        can_tx: bxcan::Tx<Can<CAN1>>,
        can_rx: bxcan::Rx<Can<CAN1>>,

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
            /*.sysclk(64.MHz())
            .hclk(64.MHz())
            .pclk1(8.MHz())
            .pclk2(64.MHz())*/
            .freeze(&mut flash.acr);

        let mut afio = ctx.device.AFIO.constrain();

        let mono = Systick::new(ctx.core.SYST, 8_000_000);
        //let mut delay = delay::Delay::new(ctx.core.SYST, 36_000_000);

        let mut gpioa = ctx.device.GPIOA.split();
        let mut led = gpioa.pa2.into_push_pull_output(&mut gpioa.crl);
        let led2 = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);

        // USART1
        let tx = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);
        let rx = gpioa.pa10;

        let mut serial = Serial::usart1(
            //dev_p.USART1,
            ctx.device.USART1,
            (tx, rx),
            &mut afio.mapr,
            Config::default().baudrate(115200.bps()),
            clocks,
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
            .enable_bank(0, bxcan::filter::Mask32::accept_all());
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

        let (can_tx, can_rx) = can.split();

        let can_tx_queue = BinaryHeap::new();

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
            enqueue_frame(
                can_tx_queue,
                PriorityFrame(canbus_common::frames::Frame::Serial(
                    Type::Data(DEVICE_SERIAL),
                )),
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
                }

                if fw_upload.finished {
                    fw_upload.finished = false;
                    fw_upload.data.reset();
                    //hprintln!("finished");
                    cx.shared.serial.lock(|serial| {
                        write!(serial, "finished\r\n").unwrap();
                    });
                }

                if fw_upload.paused {
                    fw_upload.paused = false;

                    cx.shared.can_tx_queue.lock(|can_tx_queue| {
                        enqueue_frame(
                            can_tx_queue,
                            PriorityFrame(canbus_common::frames::Frame::FirmwareUploadPause(
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
                let flash_size = u32::from_be_bytes(*unsafe { &*(begin as *const [u8; 4]) }); // without len and crc
                                                                                              //let flash_size = unsafe { core::slice::from_raw_parts(&*(ptri as *const u8), 4) };

                let version = canbus_common::frames::version::Version::from(*unsafe {
                    &*((begin + 4) as *const [u8; 8])
                });

                //hprintln!("flash size {}", flash_size);
                cx.shared.serial.lock(|serial| {
                    write!(serial, "flash size {}\r\n", flash_size).unwrap();
                });
                // len+version+flash
                let flash_data = unsafe {
                    core::slice::from_raw_parts(&*(begin as *const u8), (flash_size + 4) as usize)
                };

                let crc = u32::from_be_bytes(*unsafe {
                    &*((begin + flash_data.len() as u32) as *const [u8; 4])
                });

                let mut hasher = crc32fast::Hasher::new();
                hasher.update(flash_data);
                let crc_calculated = hasher.finalize();

                cx.shared.can_tx_queue.lock(|can_tx_queue| {
                    enqueue_frame(
                        can_tx_queue,
                        PriorityFrame(canbus_common::frames::Frame::PendingFirmwareVersion(
                            Type::Data(match crc == crc_calculated {
                                true => Some(version),
                                false => {
                                    //hprintln!("wrong crc {} {}", crc, crc_calculated);
                                    cx.shared.serial.lock(|serial| {
                                        write!(serial, "wrong crc {} {}\r\n", crc, crc_calculated).unwrap();
                                    });
                                    None
                                }
                            }),
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

    // This ISR is triggered by each finished frame transmission.
    #[task(binds = USB_HP_CAN_TX, local = [can_tx], shared = [can_tx_queue, tx_count, led2, dyn_id, serial])]
    fn can_tx(mut cx: can_tx::Context) {
        let tx = cx.local.can_tx;
        let mut tx_queue = cx.shared.can_tx_queue;
        //let _tx_count = cx.shared.tx_count;

        cx.shared.serial.lock(|serial| {
            write!(serial, "can_tx\r\n").unwrap();
        });

        tx.clear_interrupt_flags();

        cx.shared.led2.lock(|_led| {
            //led.set_high();
        });

        // There is now a free mailbox. Try to transmit pending frames until either
        // the queue is empty or transmission would block the execution of this ISR.
        tx_queue.lock(|tx_queue| {
            //let mut serial = cx.shared.serial;
            //hprintln!("tx_queue {}", tx_queue.len());
            while let Some(frame) = tx_queue.peek() {
                //hprintln!("tx_queue1");
                let sub_id = cx.shared.dyn_id.lock(|v| *v);
                /*hprintln!("tx_queue12");
                cx.shared.serial.lock(|serial| {
                    write!(serial, "tx_queue12: {:?} {:?}\r\n", sub_id, frame).unwrap();
                    //nb::block!(serial.write_str("fdddddddddd"));
                    //write!(serial, "123456789\r\n").unwrap();
                });*/
                let f = frame.to_bx_frame(sub_id);
                //hprintln!("tx_queue123");
                let t = tx.transmit(&f);
                //hprintln!("tx_queue1234");
                match t {
                    Ok(status) => match status.dequeued_frame() {
                        None => {
                            // Frame was successfully placed into a transmit buffer.
                            tx_queue.pop();
                        }
                        Some(pending_frame) => {
                            //hprintln!("enqueue_frame pending");
                            // A lower priority frame was replaced with our high priority frame.
                            // Put the low priority frame back in the transmit queue.
                            tx_queue.pop();

                            let f = PriorityFrame::from_bxcan_frame(pending_frame).unwrap();
                            enqueue_frame(tx_queue, f);
                        }
                    },
                    Err(nb::Error::WouldBlock) => break,
                    Err(e) => {
                        //hprintln!("Err(e) {:?}", e);
                        unreachable!()
                    }
                }
            }
        });
    }

    #[task(binds = USB_LP_CAN_RX0, local = [can_rx], shared = [can_tx_queue, led2, dyn_id, fw_upload, pending_fw_version_required, serial])]
    fn can_rx0(mut cx: can_rx0::Context) {
        // Echo back received packages with correct priority ordering.
        cx.shared.serial.lock(|serial| {
            write!(serial, "can_rx0\r\n").unwrap();
        });

        //let mut dyn_id = cx.shared.dyn_id;
        let mut can_tx_queue = cx.shared.can_tx_queue;

        loop {
            match cx.local.can_rx.receive() {
                Ok(frame) => {
                    cx.shared.led2.lock(|_led| {
                        //led.set_high();
                    });

                    let id_is_ok = true;

                    match PriorityFrame::from_bxcan_frame(&frame) {
                        Ok(frame) => match frame.0 {
                            canbus_common::frames::Frame::Serial(serial) => match serial {
                                Type::Remote => {
                                    can_tx_queue.lock(|can_tx_queue| {
                                        enqueue_frame(
                                            can_tx_queue,
                                            PriorityFrame(canbus_common::frames::Frame::Serial(
                                                Type::Data(DEVICE_SERIAL),
                                            )),
                                        );
                                    });

                                    //hprintln!("send");
                                }
                                _ => {}
                            },
                            canbus_common::frames::Frame::DynId(value) => {
                                if value.serial == DEVICE_SERIAL {
                                    let crc = crc8_fast::calc(
                                        &value.dyn_id.to_be_bytes(),
                                        crc8_fast::calc(&value.serial.0, 0),
                                    );

                                    cx.shared.dyn_id.lock(|v| {
                                        *v = SubId::from([crc, value.dyn_id]);
                                    });
                                }
                            }
                            canbus_common::frames::Frame::PendingFirmwareVersion(Type::Remote)
                                if id_is_ok =>
                            {
                                cx.shared.pending_fw_version_required.lock(
                                    |pending_fw_version_required| {
                                        *pending_fw_version_required = true;
                                    },
                                )
                            }
                            canbus_common::frames::Frame::FirmwareUploadPart(value) if id_is_ok => {
                                cx.shared.serial.lock(|serial| {
                                    write!(serial, "FirmwareUploadPart: {:?}\r\n", value).unwrap();
                                });

                                cx.shared.fw_upload.lock(|fw_upload| {
                                    match fw_upload.data.put_part(value.data, value.position()) {
                                        Ok(_) => {
                                            if fw_upload.data.page_is_ready() && !fw_upload.paused {
                                                fw_upload.paused = true;

                                                can_tx_queue.lock(|can_tx_queue| {
                                                    enqueue_frame(
                                                        can_tx_queue,
                                                        PriorityFrame(canbus_common::frames::Frame::FirmwareUploadPause(fw_upload.paused)),
                                                    );
                                                });
                                            }
                                        }
                                        Err(PutPartError::NotEnoughSpace) => {
                                            if !fw_upload.paused {
                                                fw_upload.paused = true;

                                                can_tx_queue.lock(|can_tx_queue| {
                                                    enqueue_frame(
                                                        can_tx_queue,
                                                        PriorityFrame(canbus_common::frames::Frame::FirmwareUploadPause(fw_upload.paused)),
                                                    );
                                                });
                                            }
                                        }
                                        Err(PutPartError::LessOfMinPart(p)) | Err(PutPartError::MoreOfMaxPart(p)) => {
                                            can_tx_queue.lock(|can_tx_queue| {
                                                enqueue_frame(
                                                    can_tx_queue,
                                                    PriorityFrame(canbus_common::frames::Frame::FirmwareUploadPartChangePos(
                                                        canbus_common::frames::firmware::UploadPartChangePos::new(p).unwrap())
                                                    ),
                                                );
                                            });
                                        }
                                    }
                                });
                            }
                            canbus_common::frames::Frame::FirmwareUploadFinished if id_is_ok => {
                                cx.shared.fw_upload.lock(|fw_upload| {
                                    while !fw_upload.data.page_is_ready() {
                                        fw_upload
                                            .data
                                            .put_part(
                                                [0_u8; 5],
                                                fw_upload.data.loaded_parts_count(),
                                            )
                                            .unwrap();
                                    }
                                    //hprintln!("qqqqqqqqq");

                                    fw_upload.paused = true;
                                    fw_upload.finished = true;

                                    can_tx_queue.lock(|can_tx_queue| {
                                        enqueue_frame(
                                            can_tx_queue,
                                            PriorityFrame(
                                                canbus_common::frames::Frame::FirmwareUploadPause(
                                                    fw_upload.paused,
                                                ),
                                            ),
                                        );
                                    });
                                });
                            }
                            _ => {}
                        },

                        Err(_) => {
                            //hprintln!("parse_frame er");
                        }
                    }
                }
                Err(nb::Error::WouldBlock) => {
                    //hprintln!("e WouldBlock");
                    break;
                }
                Err(nb::Error::Other(_e)) => {
                    //hprintln!("rx overrun");
                    cx.shared.serial.lock(|serial| {
                        write!(serial, "rx overrun").unwrap();
                    });
                } // Ignore overrun errors.
            }
        }
    }
}
