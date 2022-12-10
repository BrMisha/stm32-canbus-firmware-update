use crate::frame_id::FrameId;
use crate::frames::firmware::{UploadPart, UploadPartChangePos};
use crate::frames::Type::{Data, Remote};

pub mod dyn_id;
pub mod firmware;
pub mod serial;
pub mod version;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Type<T> {
    Data(T),
    Remote,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Frame {
    Serial(Type<serial::Serial>),
    DynId(dyn_id::Data),
    HardwareVersion(Type<version::Version>),
    FirmwareVersion(Type<version::Version>),
    PendingFirmwareVersion(Type<Option<version::Version>>),
    FirmwareUploadPartChangePos(firmware::UploadPartChangePos),
    FirmwareUploadPause(bool),
    FirmwareUploadPart(firmware::UploadPart),
    FirmwareUploadFinished,
    FirmwareStartUpdate,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParseError {
    WrongDataSize,
    WrongData,
    UnknownId,
    RemoteFrame,
    RemovedWrongDlc,
    Other,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParserType<'a> {
    Data(&'a [u8]),
    Remote(u8),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RawType {
    Data(arrayvec::ArrayVec<u8, 8>),
    Remote(u8),
}

impl RawType {
    pub fn new_data<T: IntoIterator<Item = u8>>(array: T) -> Self {
        let mut t = arrayvec::ArrayVec::<u8, 8>::new();
        t.extend(array);
        Self::Data(t)
    }
}

impl Frame {
    pub fn parse_frame(frame_id: FrameId, data: ParserType) -> Result<Frame, ParseError> {
        match frame_id {
            FrameId::Serial => match data {
                ParserType::Remote(len) => match len {
                    5 => Ok(Frame::Serial(Remote)),
                    _l => Err(ParseError::RemovedWrongDlc),
                },
                ParserType::Data(data) => match data.len() {
                    5 => Ok(Frame::Serial(Data(serial::Serial::from(
                        <[u8; 5]>::try_from(&data[0..5]).unwrap(),
                    )))),
                    _ => Err(ParseError::WrongDataSize),
                },
            },
            FrameId::DynId => match data {
                ParserType::Remote(_) => Err(ParseError::RemoteFrame),
                ParserType::Data(data) => match data.len() {
                    6 => Ok(Frame::DynId(dyn_id::Data::from(
                        <[u8; 6]>::try_from(&data[0..6]).unwrap(),
                    ))),
                    _ => Err(ParseError::WrongDataSize),
                },
            },
            n @ FrameId::HardwareVersion | n @ FrameId::FirmwareVersion => {
                fn put(n: FrameId, v: Type<version::Version>) -> Frame {
                    match n {
                        FrameId::HardwareVersion => Frame::HardwareVersion(v),
                        FrameId::FirmwareVersion => Frame::FirmwareVersion(v),
                        _ => unreachable!(),
                    }
                }
                match data {
                    ParserType::Remote(len) => match len {
                        8 => Ok(put(n, Remote)),
                        _ => Err(ParseError::RemovedWrongDlc),
                    },
                    ParserType::Data(data) => match data.len() {
                        8 => Ok(put(
                            n,
                            Data(version::Version::from(
                                <[u8; 8]>::try_from(&data[..8]).unwrap(),
                            )),
                        )),
                        _ => Err(ParseError::RemovedWrongDlc),
                    },
                }
            }
            FrameId::PendingFirmwareVersion => match data {
                ParserType::Remote(len) => match len {
                    8 => Ok(Frame::PendingFirmwareVersion(Remote)),
                    _ => Err(ParseError::RemovedWrongDlc),
                },
                ParserType::Data(data) => match data.len() {
                    0 => Ok(Frame::PendingFirmwareVersion(Data(None))),
                    8 => Ok(Frame::PendingFirmwareVersion(Data(Some(
                        version::Version::from(<[u8; 8]>::try_from(&data[..8]).unwrap()),
                    )))),
                    _ => Err(ParseError::RemovedWrongDlc),
                },
            },
            FrameId::FirmwareUploadPartChangePos => match data {
                ParserType::Remote(_) => Err(ParseError::RemoteFrame),
                ParserType::Data(data) => match data.len() {
                    3 => Ok(Frame::FirmwareUploadPartChangePos(
                        UploadPartChangePos::from(<[u8; 3]>::try_from(&data[..3]).unwrap()),
                    )),
                    _ => Err(ParseError::WrongDataSize),
                },
            },
            FrameId::FirmwareUploadPart => match data {
                ParserType::Remote(_) => Err(ParseError::RemoteFrame),
                ParserType::Data(data) => match data.len() {
                    8 => Ok(Frame::FirmwareUploadPart(UploadPart::from(
                        <[u8; 8]>::try_from(&data[..8]).unwrap(),
                    ))),
                    _ => Err(ParseError::WrongDataSize),
                },
            },
            FrameId::FirmwareStartUpdate => match data {
                ParserType::Remote(_) => Err(ParseError::RemoteFrame),
                ParserType::Data(_) => Ok(Frame::FirmwareStartUpdate),
            },
            FrameId::FirmwareUploadPause => match data {
                ParserType::Remote(_) => Err(ParseError::RemoteFrame),
                ParserType::Data(data) => match data.len() {
                    1 => Ok(Frame::FirmwareUploadPause(data[0] != 0)),
                    _ => Err(ParseError::WrongDataSize),
                },
            },
            FrameId::FirmwareUploadFinished => match data {
                ParserType::Remote(_) => Err(ParseError::RemoteFrame),
                ParserType::Data(_) => Ok(Frame::FirmwareUploadFinished),
            },
        }
    }

    pub fn raw_frame(&self) -> (FrameId, RawType) {
        match self {
            Frame::Serial(v) => match v {
                Remote => (FrameId::Serial, RawType::Remote(5)),
                Data(v) => (FrameId::Serial, RawType::new_data(v.0)),
            },
            Frame::DynId(v) => (FrameId::DynId, RawType::new_data(<[u8; 6]>::from(*v))),
            n @ Frame::HardwareVersion(v) | n @ Frame::FirmwareVersion(v) => {
                let id = match n {
                    Frame::HardwareVersion(_) => FrameId::HardwareVersion,
                    Frame::FirmwareVersion(_) => FrameId::FirmwareVersion,
                    _ => unreachable!(),
                };
                match v {
                    Remote => (id, RawType::Remote(8)),
                    Data(v) => (id, RawType::new_data(<[u8; 8]>::from(*v))),
                }
            }
            Frame::PendingFirmwareVersion(v) => (
                FrameId::PendingFirmwareVersion,
                match v {
                    Remote => RawType::Remote(8),
                    Data(Some(v)) => RawType::new_data(<[u8; 8]>::from(*v)),
                    Data(None) => RawType::new_data([0_u8; 0]),
                },
            ),
            Frame::FirmwareUploadPartChangePos(v) => (
                FrameId::FirmwareUploadPartChangePos,
                RawType::new_data(<[u8; 3]>::from(*v)),
            ),
            Frame::FirmwareUploadPause(v) => (
                FrameId::FirmwareUploadPause,
                RawType::new_data([u8::from(*v)]),
            ),
            Frame::FirmwareUploadPart(v) => (
                FrameId::FirmwareUploadPart,
                RawType::new_data(<[u8; 8]>::from(*v)),
            ),
            Frame::FirmwareStartUpdate => (FrameId::FirmwareStartUpdate, RawType::new_data([])),
            Frame::FirmwareUploadFinished => {
                (FrameId::FirmwareUploadFinished, RawType::new_data([]))
            }
        }
    }

    #[inline]
    pub fn id(&self) -> FrameId {
        match self {
            Frame::Serial(_) => FrameId::Serial,
            Frame::DynId(_) => FrameId::DynId,
            Frame::HardwareVersion(_) => FrameId::HardwareVersion,
            Frame::FirmwareVersion(_) => FrameId::FirmwareVersion,
            Frame::PendingFirmwareVersion(_) => FrameId::PendingFirmwareVersion,
            Frame::FirmwareUploadPartChangePos(_) => FrameId::FirmwareUploadPartChangePos,
            Frame::FirmwareUploadPause(_) => FrameId::FirmwareUploadPause,
            Frame::FirmwareUploadPart(_) => FrameId::FirmwareUploadPart,
            Frame::FirmwareStartUpdate => FrameId::FirmwareStartUpdate,
            Frame::FirmwareUploadFinished => FrameId::FirmwareUploadFinished,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_id::FrameId;
    use crate::frames::ParserType;

    #[test]
    fn serial() {
        assert_eq!(
            Frame::parse_frame(FrameId::Serial, ParserType::Remote(5),),
            Ok(Frame::Serial(Remote))
        );

        assert_eq!(
            Frame::Serial(Remote).raw_frame(),
            (FrameId::Serial, RawType::Remote(5))
        );

        assert_eq!(
            Frame::parse_frame(FrameId::Serial, ParserType::Data(&[1, 2, 3, 4, 5])),
            Ok(Frame::Serial(Data(serial::Serial::from([1, 2, 3, 4, 5]))))
        );

        assert_eq!(
            Frame::Serial(Data(serial::Serial::from([1, 2, 3, 4, 5]))).raw_frame(),
            (FrameId::Serial, RawType::new_data([1, 2, 3, 4, 5]))
        );
    }

    #[test]
    fn dyn_id() {
        assert_eq!(
            Frame::parse_frame(FrameId::DynId, ParserType::Remote(0),),
            Err(ParseError::RemoteFrame)
        );

        assert_eq!(
            Frame::parse_frame(FrameId::DynId, ParserType::Data(&[1, 2, 3, 4, 5, 80]),),
            Ok(Frame::DynId(dyn_id::Data::new(
                serial::Serial::from([1, 2, 3, 4, 5]),
                80
            )))
        );

        assert_eq!(
            Frame::DynId(dyn_id::Data::new(serial::Serial::from([1, 2, 3, 4, 5]), 55)).raw_frame(),
            (FrameId::DynId, RawType::new_data([1, 2, 3, 4, 5, 55]))
        );
    }

    #[test]
    fn version() {
        fn none(id: FrameId, res: Frame) {
            assert_eq!(Frame::parse_frame(id, ParserType::Remote(8)), Ok(res));
            assert_eq!(res.raw_frame(), (id, RawType::Remote(8)));
        }
        none(FrameId::FirmwareVersion, Frame::FirmwareVersion(Remote));
        none(FrameId::HardwareVersion, Frame::HardwareVersion(Remote));
        none(
            FrameId::PendingFirmwareVersion,
            Frame::PendingFirmwareVersion(Remote),
        );

        fn data(v: version::Version, id: FrameId, res: Frame) {
            assert_eq!(
                Frame::parse_frame(id, ParserType::Data(&<[u8; 8]>::from(v))),
                Ok(res)
            );

            assert_eq!(res.raw_frame(), (id, RawType::new_data(<[u8; 8]>::from(v))));
        }
        let v = version::Version {
            major: 1,
            minor: 2,
            path: 3,
            build: 9864,
        };
        data(v, FrameId::FirmwareVersion, Frame::FirmwareVersion(Data(v)));
        data(v, FrameId::HardwareVersion, Frame::HardwareVersion(Data(v)));
        data(
            v,
            FrameId::PendingFirmwareVersion,
            Frame::PendingFirmwareVersion(Data(Some(v))),
        );

        assert_eq!(
            Frame::parse_frame(
                FrameId::PendingFirmwareVersion,
                ParserType::Data(&[0_u8; 0])
            ),
            Ok(Frame::PendingFirmwareVersion(Data(None)))
        );

        assert_eq!(
            Frame::PendingFirmwareVersion(Data(Some(v))).raw_frame(),
            (
                FrameId::PendingFirmwareVersion,
                RawType::new_data(<[u8; 8]>::from(v))
            )
        );
    }

    #[test]
    fn firmware_upload_part_change_pos() {
        assert_eq!(
            Frame::parse_frame(FrameId::DynId, ParserType::Data(&[1, 2, 3, 4]),),
            Err(ParseError::WrongDataSize)
        );

        assert_eq!(
            Frame::parse_frame(
                FrameId::FirmwareUploadPartChangePos,
                ParserType::Data(&[0x01, 0x02, 0x03]),
            ),
            Ok(Frame::FirmwareUploadPartChangePos(
                firmware::UploadPartChangePos::new(0x010203usize).unwrap()
            ))
        );

        assert_eq!(
            Frame::FirmwareUploadPartChangePos(
                firmware::UploadPartChangePos::new(0x010203usize).unwrap()
            )
            .raw_frame(),
            (
                FrameId::FirmwareUploadPartChangePos,
                RawType::new_data([0x01, 0x02, 0x03])
            )
        );
    }

    #[test]
    fn firmware_upload_part() {
        assert_eq!(
            Frame::parse_frame(
                FrameId::FirmwareUploadPart,
                ParserType::Data(&[0x01, 0x02, 0x03, 1, 2, 3, 4])
            ),
            Err(ParseError::WrongDataSize)
        );

        assert_eq!(
            Frame::parse_frame(FrameId::FirmwareUploadPart, ParserType::Remote(1)),
            Err(ParseError::RemoteFrame)
        );

        assert_eq!(
            Frame::parse_frame(
                FrameId::FirmwareUploadPart,
                ParserType::Data(&[0x01, 0x02, 0x03, 1, 2, 3, 4, 5])
            ),
            Ok(Frame::FirmwareUploadPart(
                firmware::UploadPart::new(0x010203usize, [1, 2, 3, 4, 5]).unwrap()
            ))
        );

        assert_eq!(
            Frame::FirmwareUploadPart(
                firmware::UploadPart::new(0x010203usize, [1, 2, 3, 4, 5]).unwrap()
            )
            .raw_frame(),
            (
                FrameId::FirmwareUploadPart,
                RawType::new_data([0x01, 0x02, 0x03, 1, 2, 3, 4, 5])
            )
        );
    }

    #[test]
    fn firmware_start_update() {
        assert_eq!(
            Frame::parse_frame(FrameId::FirmwareStartUpdate, ParserType::Remote(1)),
            Err(ParseError::RemoteFrame)
        );

        assert_eq!(
            Frame::parse_frame(FrameId::FirmwareStartUpdate, ParserType::Data(&[])),
            Ok(Frame::FirmwareStartUpdate)
        );

        assert_eq!(
            Frame::FirmwareStartUpdate.raw_frame(),
            (FrameId::FirmwareStartUpdate, RawType::new_data([]))
        );
    }

    #[test]
    fn firmware_upload_pause() {
        assert_eq!(
            Frame::parse_frame(FrameId::FirmwareUploadPause, ParserType::Remote(1)),
            Err(ParseError::RemoteFrame)
        );

        assert_eq!(
            Frame::parse_frame(FrameId::FirmwareUploadPause, ParserType::Data(&[1])),
            Ok(Frame::FirmwareUploadPause(true))
        );

        assert_eq!(
            Frame::parse_frame(FrameId::FirmwareUploadPause, ParserType::Data(&[0])),
            Ok(Frame::FirmwareUploadPause(false))
        );

        assert_eq!(
            Frame::FirmwareUploadPause(false).raw_frame(),
            (FrameId::FirmwareUploadPause, RawType::new_data([0]))
        );

        assert_eq!(
            Frame::FirmwareUploadPause(true).raw_frame(),
            (FrameId::FirmwareUploadPause, RawType::new_data([1]))
        );
    }
}
