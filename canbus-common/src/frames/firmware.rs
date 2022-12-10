use core::ops::{Deref, DerefMut};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct UploadPartChangePos(usize);

impl UploadPartChangePos {
    pub const MAX: usize = 0xFFFFFF;

    pub fn new(pos: usize) -> Option<Self> {
        match pos {
            0..=Self::MAX => Some(Self(pos)),
            _ => None,
        }
    }

    #[inline]
    pub fn pos(&self) -> usize {
        self.0
    }
}

impl From<[u8; 3]> for UploadPartChangePos {
    fn from(val: [u8; 3]) -> Self {
        let mut ar: [u8; 4] = [0, 0, 0, 0];
        ar[1..].clone_from_slice(val[..].try_into().unwrap());
        UploadPartChangePos(u32::from_be_bytes(ar) as usize)
    }
}

impl From<UploadPartChangePos> for [u8; 3] {
    fn from(v: UploadPartChangePos) -> Self {
        let arr: [u8; 4] = (v.0 as u32).to_be_bytes();
        arr[1..].try_into().unwrap()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct UploadPart {
    position: usize,
    pub data: [u8; 5],
}

impl UploadPart {
    pub fn new(position: usize, data: [u8; 5]) -> Option<Self> {
        match position {
            0..=UploadPartChangePos::MAX => Some(Self { position, data }),
            _ => None,
        }
    }

    #[inline]
    pub fn position(&self) -> usize {
        self.position
    }
}

impl From<[u8; 8]> for UploadPart {
    fn from(val: [u8; 8]) -> Self {
        UploadPart {
            position: {
                // use first 3 bytes
                let mut ar: [u8; 4] = [0, 0, 0, 0];
                ar[1..].clone_from_slice(val[..3].try_into().unwrap());
                u32::from_be_bytes(ar) as usize
            },
            data: val[3..].try_into().unwrap(),
        }
    }
}

impl From<UploadPart> for [u8; 8] {
    fn from(v: UploadPart) -> Self {
        let mut ar: [u8; 8] = Default::default();
        ar[..3].clone_from_slice((v.position as u32).to_be_bytes()[1..].try_into().unwrap());
        ar[3..].clone_from_slice(&v.data);

        ar
    }
}

impl Deref for UploadPart {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.data
    }
}

impl DerefMut for UploadPart {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl AsRef<[u8]> for UploadPart {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl AsMut<[u8]> for UploadPart {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.deref_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_part_change_pos() {
        assert_eq!(UploadPartChangePos::new(0).unwrap().pos(), 0);
        assert_eq!(
            UploadPartChangePos::new(125468usize).unwrap().pos(),
            125468usize
        );
        assert_eq!(UploadPartChangePos::new(0xFFFFFFusize + 1), None);

        let v = UploadPartChangePos::new(15000000).unwrap();
        let arr: [u8; 3] = v.into();
        assert_eq!(arr, [0xE4, 0xE1, 0xC0]);
        assert_eq!(
            UploadPartChangePos::from(arr).pos(),
            15000000,
            "arr is {:#04x} {:#04x} {:#04x}",
            arr[0],
            arr[1],
            arr[2]
        );
    }

    #[test]
    fn upload_part() {
        assert_eq!(UploadPart::new(0xFFFFFFusize + 1, [1, 2, 3, 4, 5]), None);

        let p = UploadPart::new(0xFFF1FFusize, [1, 2, 3, 4, 5]).unwrap();
        assert_eq!(p.deref(), [1, 2, 3, 4, 5]);
        assert_eq!(p.position, 0xFFF1FFusize);

        let p = UploadPart::from([0x01, 0x02, 0x03, 1, 2, 3, 4, 5]);
        assert_eq!(p.data, [1, 2, 3, 4, 5]);
        assert_eq!(p.position, 0x010203usize);

        assert_eq!(<[u8; 8]>::from(p), [0x01, 0x02, 0x03, 1, 2, 3, 4, 5]);
    }
}
