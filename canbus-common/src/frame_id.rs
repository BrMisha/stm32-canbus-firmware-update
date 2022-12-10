use num_traits::FromPrimitive;
use num_traits::ToPrimitive;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SubId(pub u16);

impl SubId {
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }

    pub fn split(&self) -> [u8; 2] {
        self.0.to_be_bytes()
    }
}

impl From<SubId> for [u8; 2] {
    fn from(v: SubId) -> Self {
        v.split()
    }
}

impl From<[u8; 2]> for SubId {
    fn from(v: [u8; 2]) -> Self {
        Self(u16::from_be_bytes(v))
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, enum_primitive_derive::Primitive)]
pub enum FrameId {
    Serial = 8000,
    DynId = 8001,

    HardwareVersion = 8010,

    FirmwareVersion = 8020,
    PendingFirmwareVersion = 8021,
    FirmwareUploadPartChangePos = 8025, // to host
    FirmwareUploadPause = 8026,         // to host
    FirmwareUploadPart = 8028,          // from host
    FirmwareUploadFinished = 8029,         // from host
    FirmwareStartUpdate = 8030,         // from host
}

impl FrameId {
    const LENGTH_BIT: usize = 13;
    pub const fn max_id() -> u16 {
        (2usize.pow(Self::LENGTH_BIT as u32) as u16) - 1
    }

    pub fn try_from_u16(value: u16) -> Option<Self> {
        if value <= Self::max_id() {
            Self::from_u16(value)
        } else {
            None
        }
    }

    #[inline]
    pub fn try_from_u32_with_sub_id(value: u32) -> Option<(Self, SubId)> {
        FrameId::try_from_u16((value as u16) & Self::max_id())
            .map(|v| (v, Self::extract_sub_id(value)))
    }

    #[inline]
    pub fn extract_sub_id(value: u32) -> SubId {
        SubId(((value >> Self::LENGTH_BIT) & 0xFFFF) as u16)
    }

    #[inline]
    pub fn as_raw(&self, sub_id: SubId) -> u32 {
        ((sub_id.0 as u32) << Self::LENGTH_BIT) | ((self.to_u16().unwrap() & Self::max_id()) as u32)
    }
}

impl From<FrameId> for u16 {
    fn from(v: FrameId) -> Self {
        v.to_u16().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_id() {
        assert_eq!(FrameId::from_u16(65535), None);

        assert_eq!(FrameId::from_u16(8000), Some(FrameId::Serial));

        assert_eq!(
            FrameId::extract_sub_id((4587u32 << 13) | 8000u32),
            SubId(4587)
        );

        assert_eq!(
            FrameId::try_from_u32_with_sub_id((4587u32 << 13) | 8000u32),
            Some((FrameId::Serial, SubId(4587)))
        );

        assert_eq!(
            FrameId::Serial.as_raw(SubId(4587)),
            (4587u32 << 13) | 8000u32
        );
    }
}
