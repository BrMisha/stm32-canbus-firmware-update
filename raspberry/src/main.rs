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


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
enum Args {
    /*Get {
        #[clap(long)]
        tt: String
    },*/
    ShowSerials,
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
        }
    }
    return Ok(());


    let can_receiver = can.subscribe();
    can.write_frame(
        &canbus_common::frames::Frame::Serial(canbus_common::frames::Type::Remote),
        canbus_common::frame_id::SubId(0),
    )
        .map_err(util::Error::Socket)?
        .await?;
    let res = util::get_serial(can_receiver).await?;
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
            .map_err(util::Error::Socket)?
            .await?;

        let can_receiver = can.subscribe();
        can.write_frame(
            &canbus_common::frames::Frame::Serial(canbus_common::frames::Type::Remote),
            canbus_common::frame_id::SubId(0),
        )
            .map_err(util::Error::Socket)?
            .await?;

        let res = util::get_serial(can_receiver).await?;
        println!("Serial: {:?}", res);
        if res.1.split()[1] != 10 {
            return Err(util::Error::Other("Unable to set dyn_id".to_string()));
        }
        sub_id = res.1;
    }

    //let file = std::fs::read("/home/pi/file.txt").unwrap();
    //let file = std::fs::read("/home/pi/file2.jpg").unwrap();
    /*let file = std::fs::read("/home/pi/file3.jpg").unwrap();

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
*/
    let timer = std::time::Instant::now();

    let data = std::fs::read("/home/pi/app.bin").unwrap();
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
    println!("p ver {:?}", res);

    if let Ok((Some(v), _)) = res {
        can.write_frame(
            &canbus_common::frames::Frame::FirmwareStartUpdate,
            sub_id,
        )
            .map_err(util::Error::Socket)?
            .await?;
    }

    Ok(())
}
