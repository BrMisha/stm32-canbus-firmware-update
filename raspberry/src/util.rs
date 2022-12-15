use tokio::select;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::{sleep, Duration};
use canbus_common::{
    frames,
    frame_id
};

#[derive(Debug)]
pub enum Error {
    Socket(tokio_socketcan::Error),
    Io(std::io::Error),
    Other(String),
}

pub async fn wait_data<T, O: Fn(&canbus_common::frames::Frame) -> Option<T>>(
    mut socket_rx: Receiver<(frames::Frame, frame_id::SubId)>,
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

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub async fn get_serial(
    can_receiver: Receiver<(frames::Frame, frame_id::SubId)>,
) -> Result<
    (
        frames::serial::Serial,
        frame_id::SubId,
    ),
    Error,
> {
    wait_data(can_receiver, |frame| match frame {
        frames::Frame::Serial(canbus_common::frames::Type::Data(value)) => {
            Some(*value)
        }
        _ => None,
    })
        .await
        .ok_or(Error::Other("Request serial".to_string()))
}