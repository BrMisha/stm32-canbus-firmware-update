use futures_util::StreamExt;
use tokio_socketcan::{CANSocket};
use canbus_common::frame_id::SubId;
use canbus_common::frames::Frame;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;

pub fn from_can_frame(f: &socketcan::CANFrame) -> Result<(canbus_common::frames::Frame, canbus_common::frame_id::SubId), ()> {
    f.is_extended().then(|| ()).ok_or(())?;
    let id =
        canbus_common::frame_id::FrameId::try_from_u32_with_sub_id(f.id() & socketcan::EFF_MASK).ok_or(())?;

    let res = canbus_common::frames::Frame::parse_frame(
        id.0,
        match f.is_rtr() {
            false => canbus_common::frames::ParserType::Data(f.data()),
            true => canbus_common::frames::ParserType::Remote(f.data().len() as u8),
        },
    )
        .map_err(|_| ())?;

    Ok((res, id.1))
}

pub fn to_can_frame(frame: &canbus_common::frames::Frame, sub_id: canbus_common::frame_id::SubId) -> socketcan::CANFrame {
    let raw = frame.raw_frame();
    let raw_id = raw.0.as_raw(sub_id);

    match raw.1 {
        canbus_common::frames::RawType::Data(v) => socketcan::CANFrame::new(raw_id, v.as_slice(), false, false).unwrap(),
        canbus_common::frames::RawType::Remote(len) => {
            let mut v = Vec::<u8>::with_capacity(len as usize);
            for _ in 0..len {
                v.push(0);
            }
            socketcan::CANFrame::new(raw_id, v.as_slice(), true, false).unwrap()
        }
    }
}

pub struct CanBus {
    handler: JoinHandle<()>,
    broadcast_s: broadcast::Sender<(canbus_common::frames::Frame, canbus_common::frame_id::SubId)>,
    socket_tx: CANSocket,
}

impl CanBus {
    pub fn open(ifname: &str) -> Result<CanBus, tokio_socketcan::Error> {
        let socket_tx = CANSocket::open(ifname)?;
        let socket_rx = CANSocket::open(ifname)?;

        let broadcast = broadcast::channel(1000).0;

        let t = tokio::spawn({
            Self::receiving(socket_rx, broadcast.clone())
        });

        Ok(Self {
            handler: t,
            broadcast_s: broadcast,
            socket_tx,
        })
    }

    async fn receiving(mut socket: CANSocket, sender: broadcast::Sender<(canbus_common::frames::Frame, canbus_common::frame_id::SubId)>) {
        loop {
            match socket.next().await {
                Some(Ok(v)) => {
                    if let Ok(v) = from_can_frame(&v) {
                        let _ = sender.send(v);
                    }
                }
                e => {println!("receiving {:?}", e)}
            };
        }
    }

    pub fn subscribe(&self) -> Receiver<(Frame, SubId)> {
        self.broadcast_s.subscribe()
    }

    pub fn write_frame(&self, frame: &canbus_common::frames::Frame, sub_id: canbus_common::frame_id::SubId) -> Result<tokio_socketcan::CANWriteFuture, tokio_socketcan::Error> {
        self.socket_tx.write_frame(to_can_frame(frame, sub_id))
    }
}
