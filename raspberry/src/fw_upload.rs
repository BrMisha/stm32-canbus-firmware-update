use canbus_common::frame_id::SubId;
use futures_util::{StreamExt, TryFutureExt};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;
use tokio::{select};
use tokio::sync::{Mutex, watch};

use tokio_socketcan::CANSocket;
use crate::{can_bus, Error};

pub async fn upload(can: &can_bus::CanBus, sub_id: SubId, file: &[u8]) {
    let part: Arc<Mutex<usize>> = Default::default();
    let pause= watch::channel(false).0;
    let mut pause_sub = pause.subscribe();

    let res = select! {
        res = {
            upload_parts(sub_id, file, &can, part.clone(), pause.subscribe())
        } => res,
        res = {
            poll_rx(sub_id.clone(), can.subscribe(), part.clone(), pause)
        } => res
    };

    // wait pause finish
    select! {
        res = async {
            loop {
                let p = *pause_sub.borrow();
                match p {
                    true => {drop(pause_sub.changed().await);},
                    false => {break;}
                }
            }
        } => res,
        _timeout = tokio::time::sleep(Duration::from_secs(20)) => {}
    }
}

async fn poll_rx(
    id: SubId,
    mut socket: tokio::sync::broadcast::Receiver<(canbus_common::frames::Frame, SubId)>,
    part: Arc<Mutex<usize>>,
    pause: watch::Sender<bool>,
) -> Result<(), crate::Error> {

    while let Ok(frame) = socket.recv().await {
        if id == frame.1 {
            match frame.0 {
                canbus_common::frames::Frame::FirmwareUploadPartChangePos(value) => {
                    println!("change pos {:?}", value);
                    *part.lock().await = value.pos();
                }
                canbus_common::frames::Frame::FirmwareUploadPause(value) => {
                    println!("pause {:?}", value);
                    pause.send(value).unwrap();
                }
                _ => {}
            }
        }
    }

    Ok(())
}

async fn upload_parts(
    id: SubId,
    file: &[u8],
    socket: &can_bus::CanBus,
    part: Arc<Mutex<usize>>,
    mut pause: watch::Receiver<bool>,
) -> Result<(), crate::Error> {
    println!("upload_parts {:?}", std::thread::current());
    let file_len = file.len();
    println!("file_len {:?}", file_len);

    loop {
        let p = *pause.borrow();
        match p {
            true => {
                // wait change
                drop(pause.changed().await);
            }
            false => {
                // get part
                let mut part = part.lock().await;
                if *part >= file_len / 5 + 1 {
                    println!("{} {}", *part, file_len / 5 + 1);
                    return Ok(());
                }

                let offset = *part * 5;
                let mut data = {
                    if offset + 5 < file_len {
                        &file[offset..offset + 5]
                    } else {
                        &file[offset..]
                    }
                };

                if data.len() == 0 {
                    break Ok(());
                }

                match socket
                    .write_frame(
                        &canbus_common::frames::Frame::FirmwareUploadPart(
                            canbus_common::frames::firmware::UploadPart::new(*part, {
                                let mut buffer = [0u8; 5];
                                data.iter().zip(buffer.iter_mut()).for_each(|v| { *v.1 = *v.0; });
                                buffer
                            }).unwrap(),
                        ),
                        id,
                    )
                    .map_err(crate::Error::Socket)?
                    .await
                {
                    Ok(_ok) => {
                        println!(
                            "part {} {} {:?}",
                            *part,
                            data.len(),
                            std::str::from_utf8(data)
                        );
                        *part += 1;
                    }
                    Err(err) => match err.raw_os_error() {
                        Some(105) => {
                            println!("err 105");
                            tokio::time::sleep(Duration::from_millis(20)).await
                        }
                        _ => {
                            println!(
                                "ee {:?} {:?} {:?}",
                                err.raw_os_error(),
                                err.kind(),
                                err.type_id()
                            );
                            return Err(crate::Error::Io(err))
                        },
                    },
                }

                tokio::time::sleep(Duration::from_millis(match *part % 2 {
                    0 => 50,
                    _ => 10
                })).await
            }
        }
    }
}