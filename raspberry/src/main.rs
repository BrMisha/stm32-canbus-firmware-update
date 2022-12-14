mod can_bus;
mod fw_upload;

use canbus_common::frame_id::SubId;
use canbus_common::frames::version::Version;
use canbus_common::frames::Frame;
use futures_util::{FutureExt, StreamExt};

use tokio::select;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::{sleep, Duration};

async fn wait_data<T, O: Fn(&canbus_common::frames::Frame) -> Option<T>>(
    mut socket_rx: Receiver<(Frame, SubId)>,
    comparator: O,
) -> Option<(T, canbus_common::frame_id::SubId)> {
    select! {
        res = async {
            loop {
                match socket_rx.recv().await {
                    Ok(frame) => {
                        let res = comparator(&frame.0);
                        if res.is_some() {
                            return res.map(|o| {(o, frame.1)});
                        }
                    },
                    Err(RecvError::Closed) => {
                        return None
                    }
                    Err(RecvError::Lagged(l)) => {
                        println!("Lagged {}", l);
                        return None
                    }
                }
            }
            None
        } => res,
        _timeout = sleep(Duration::from_millis(2000)) => None
    }
}

#[derive(Debug)]
pub enum Error {
    Socket(tokio_socketcan::Error),
    Io(std::io::Error),
    Other(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let can = can_bus::CanBus::open("can0").unwrap();

    async fn get_serial(
        can_receiver: Receiver<(Frame, SubId)>,
    ) -> Result<
        (
            canbus_common::frames::serial::Serial,
            canbus_common::frame_id::SubId,
        ),
        Error,
    > {
        wait_data(can_receiver, |frame| match frame {
            canbus_common::frames::Frame::Serial(canbus_common::frames::Type::Data(value)) => {
                Some(*value)
            }
            _ => None,
        })
        .await
        .ok_or(Error::Other("Request serial".to_string()))
    }

    let can_receiver = can.subscribe();
    can.write_frame(
        &canbus_common::frames::Frame::Serial(canbus_common::frames::Type::Remote),
        canbus_common::frame_id::SubId(0),
    )
    .map_err(Error::Socket)?
    .await?;
    let res = get_serial(can_receiver).await?;
    println!("Serial: {:?}", res);

    let mut sub_id = res.1;
    if res.1.split()[1] == 0 {
        println!("Attempt to set dyn_id");
        can.write_frame(
            &canbus_common::frames::Frame::DynId(canbus_common::frames::dyn_id::Data::new(
                res.0, 10,
            )),
            canbus_common::frame_id::SubId(0),
        )
        .map_err(Error::Socket)?
        .await?;

        let can_receiver = can.subscribe();
        can.write_frame(
            &canbus_common::frames::Frame::Serial(canbus_common::frames::Type::Remote),
            canbus_common::frame_id::SubId(0),
        )
        .map_err(Error::Socket)?
        .await?;

        let res = get_serial(can_receiver).await?;
        println!("Serial: {:?}", res);
        if res.1.split()[1] != 10 {
            return Err(Error::Other("Unable to set dyn_id".to_string()));
        }
        sub_id = res.1;
    }

    //let file = std::fs::read("/home/pi/file.txt").unwrap();
    //let file = std::fs::read("/home/pi/file2.jpg").unwrap();
    let file = std::fs::read("/home/pi/file3.jpg").unwrap();

    let version = <[u8; 8]>::from(Version {
        major: 1,
        minor: 2,
        path: 3,
        build: 4,
    });

    let mut data = Vec::<u8>::new();
    //println!("dd {:?} {}", ((version.len() + file.len()) as u32).to_be_bytes(), ((version.len() + file.len()) as u32));
    data.extend(((version.len() + file.len()) as u32).to_be_bytes()); // add len
    data.extend(version);
    data.extend(&file);

    let crc = crc32c_hw::compute(&data);
    println!("crc {}", crc);
    data.extend(crc.to_be_bytes());

    let timer = std::time::Instant::now();
    /*fw_upload::upload(&can, sub_id, &data).await;

    can.write_frame(
        &canbus_common::frames::Frame::FirmwareUploadFinished,
        sub_id,
    )
    .map_err(Error::Socket)?
    .await?;*/

    println!("upload finish {:?}", timer.elapsed());
    //sleep(Duration::from_millis(10000)).await;

    println!("rq version");
    let can_receiver = can.subscribe();
    can.write_frame(
        &canbus_common::frames::Frame::PendingFirmwareVersion(canbus_common::frames::Type::Remote),
        sub_id,
    )
    .map_err(Error::Socket)?
    .await?;

    let res = wait_data(can_receiver, |frame| {
        println!("frame__ {:?}", frame);
        match frame {
            canbus_common::frames::Frame::PendingFirmwareVersion(
                canbus_common::frames::Type::Data(value),
            ) => Some(*value),
            _ => None,
        }
    })
    .await
    .ok_or(Error::Other("Request pending version".to_string()));
    println!("p ver {:?}", res);

    Ok(())
}
