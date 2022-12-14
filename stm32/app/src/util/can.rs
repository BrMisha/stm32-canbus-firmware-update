use stm32f1xx_hal::pac::Interrupt;
use core::cmp::Ordering;
use num_traits::ToPrimitive;
use heapless::binary_heap;
use canbus_common::{
    frames,
    frame_id
};
use helpers::firmware_update;
use rtic::mutex_prelude::*;
use crate::app::{can_rx0, can_tx};
use core::fmt::Write;

#[derive(Debug)]
pub struct PriorityFrame(pub frames::Frame);

impl PriorityFrame {
    pub fn to_bx_frame(&self, sub_id: frame_id::SubId) -> bxcan::Frame {
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

pub fn enqueue_frame(queue: &mut binary_heap::BinaryHeap<PriorityFrame, binary_heap::Max, 16>, frame: PriorityFrame) {
    if let Err(_e) = queue.push(frame) {
        //hprintln!("push err {:?}", e);
        return;
    }
    rtic::pend(Interrupt::USB_HP_CAN_TX);
}


pub fn can_rx0(mut cx: can_rx0::Context) {
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
                            frames::Type::Remote => {
                                can_tx_queue.lock(|can_tx_queue| {
                                    enqueue_frame(
                                        can_tx_queue,
                                        PriorityFrame(canbus_common::frames::Frame::Serial(
                                            frames::Type::Data(crate::DEVICE_SERIAL),
                                        )),
                                    );
                                });

                                //hprintln!("send");
                            }
                            _ => {}
                        },
                        canbus_common::frames::Frame::DynId(value) => {
                            if value.serial == crate::DEVICE_SERIAL {
                                let crc = crc8_fast::calc(
                                    &value.dyn_id.to_be_bytes(),
                                    crc8_fast::calc(&value.serial.0, 0),
                                );

                                cx.shared.dyn_id.lock(|v| {
                                    *v = frame_id::SubId::from([crc, value.dyn_id]);
                                });
                            }
                        }
                        canbus_common::frames::Frame::PendingFirmwareVersion(frames::Type::Remote)
                        if id_is_ok =>
                            {
                                cx.shared.pending_fw_version_required.lock(
                                    |pending_fw_version_required| {
                                        *pending_fw_version_required = true;
                                    },
                                )
                            }
                        canbus_common::frames::Frame::FirmwareUploadPart(value) if id_is_ok => {
                            /*cx.shared.serial.lock(|serial| {
                                write!(serial, "FirmwareUploadPart: {:?}\r\n", value).unwrap();
                            });*/

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
                                    Err(firmware_update::PutPartError::NotEnoughSpace) => {
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
                                    Err(firmware_update::PutPartError::LessOfMinPart(p)) | Err(firmware_update::PutPartError::MoreOfMaxPart(p)) => {
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
                        canbus_common::frames::Frame::FirmwareStartUpdate if id_is_ok => {
                            cx.shared.fw_upload.lock(|fw_upload| {
                                match fw_upload.has_pending_fw {
                                    true => {
                                        cx.shared.serial.lock(|serial| {
                                            write!(serial, "Reboot to upgrade...\r\n").unwrap();
                                        });
                                        cortex_m::peripheral::SCB::sys_reset();
                                    }
                                    false => {
                                        cx.shared.serial.lock(|serial| {
                                            write!(serial, "Has no pending fw !!!\r\n").unwrap();
                                        });
                                    }
                                }
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
                    write!(serial, "rx overrun\r\n").unwrap();
                });
            } // Ignore overrun errors.
        }
    }
}

pub fn can_tx(mut cx: can_tx::Context) {
    let tx = cx.local.can_tx;
    let mut tx_queue = cx.shared.can_tx_queue;
    //let _tx_count = cx.shared.tx_count;

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
                Err(_e) => {
                    //hprintln!("Err(e) {:?}", e);
                    unreachable!()
                }
            }
        }
    });
}