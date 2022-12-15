mod can_bus;
mod fw_upload;
mod util;

use canbus_common::frame_id::SubId;
use canbus_common::frames::version::Version;
use canbus_common::frames::Frame;
use futures_util::{FutureExt, StreamExt};

use tokio::select;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::{sleep, Duration};

use clap::{arg, Parser};
use crate::util::Error;


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
enum Args {
    ShowSerials,
    UpgradeFw {
        #[clap(long)]
        file_path: String,
        #[clap(long)]
        serial: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), util::Error> {
    let args = Args::parse();
    println!("{:?}", args);

    let can = can_bus::CanBus::open("can0").unwrap();

    match args {
        Args::ShowSerials => {
            let mut can_receiver = can.subscribe();
            can.write_frame(
                &canbus_common::frames::Frame::Serial(canbus_common::frames::Type::Remote),
                canbus_common::frame_id::SubId(0),
            )
                .map_err(util::Error::Socket)?
                .await?;

            let mut list = Vec::new();

            select! {
                res = async {
                    loop {
                        match can_receiver.recv().await {
                            Ok(frame) => {
                                if let Frame::Serial(canbus_common::frames::Type::Data(v)) = frame.0 {
                                    list.push(v);
                                }
                            },
                            Err(RecvError::Closed) => break,
                            _ => {},
                        }
                    }
                    ()
                } => res,
                _timeout = sleep(Duration::from_millis(2000)) => ()
            }

            println!("Serials: {:?}", list);
        },
        Args::UpgradeFw { file_path, serial } => {
            let serial = canbus_common::frames::serial::Serial::try_from(serial.as_str()).unwrap();
            let data = std::fs::read(file_path.as_str()).unwrap();

            println!("Attempt to set dyn_id");
            can.write_frame(
                &canbus_common::frames::Frame::DynId(canbus_common::frames::dyn_id::Data::new(
                    serial, 10,
                )),
                canbus_common::frame_id::SubId(0),
            )
                .map_err(util::Error::Socket)?
                .await?;

            // get serial
            let can_receiver = can.subscribe();
            can.write_frame(
                &canbus_common::frames::Frame::Serial(canbus_common::frames::Type::Remote),
                canbus_common::frame_id::SubId(0),
            )
                .map_err(util::Error::Socket)?
                .await?;
            let res = util::wait_data(can_receiver, |frame| match frame {
                Frame::Serial(canbus_common::frames::Type::Data(value)) if value == &serial => {
                    Some(())
                }
                _ => None,
            })
                .await.unwrap();

            if res.1.split()[1] != 10 {
                return Err(util::Error::Other("Unable to set dyn_id".to_string()));
            }
            let sub_id = res.1;

            let timer = std::time::Instant::now();

            fw_upload::upload(&can, sub_id, &data).await;

            can.write_frame(
                &canbus_common::frames::Frame::FirmwareUploadFinished,
                sub_id,
            )
                .map_err(util::Error::Socket)?
                .await?;

            println!("upload finish {:?}", timer.elapsed());
            //sleep(Duration::from_millis(10000)).await;

            println!("rq version");
            let can_receiver = can.subscribe();
            can.write_frame(
                &canbus_common::frames::Frame::PendingFirmwareVersion(canbus_common::frames::Type::Remote),
                sub_id,
            )
                .map_err(util::Error::Socket)?
                .await?;
            let res = util::wait_data(can_receiver, |frame| {
                println!("frame__ {:?}", frame);
                match frame {
                    canbus_common::frames::Frame::PendingFirmwareVersion(
                        canbus_common::frames::Type::Data(value),
                    ) => Some(*value),
                    _ => None,
                }
            })
                .await
                .ok_or(util::Error::Other("Request pending version".to_string()));
            match res {
                Ok((Some(v), _)) => {
                    println!("Upload successful. Start update");
                    can.write_frame(
                        &canbus_common::frames::Frame::FirmwareStartUpdate,
                        sub_id,
                    )
                        .map_err(util::Error::Socket)?
                        .await?;
                }
                Ok((None, _)) => {
                    println!("Upload error");
                }
                Err(e) => {

                }
            }
        }
    }
    Ok(())
}
