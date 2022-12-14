use core::fmt;
use core::fmt::{Debug, Display, Write};
use hex::ToHex;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Serial(pub [u8; 5]);

impl From<[u8; 5]> for Serial {
    fn from(val: [u8; 5]) -> Self {
        Self(val)
    }
}

impl From<Serial> for [u8; 5] {
    fn from(v: Serial) -> Self {
        v.0
    }
}

impl TryFrom<&str> for Serial {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() != 10 {
            return Err(());
        }

        let mut buff: [u8; 5] = Default::default();
        for i in 0..5 {
            buff[i] = u8::from_str_radix(&value[(i*2)..(i*2+2)], 16).unwrap()
        }

        Ok(Self::from(buff))
    }
}

impl From<&Serial> for heapless::String<10> {
    fn from(v: &Serial) -> Self {
        let res = v.0.encode_hex::<heapless::String<10>>();
        res
    }
}

impl Debug for Serial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(heapless::String::<10>::from(self).as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let s = Serial::from([1, 2, 3, 4, 5]);
        assert_eq!(s.0, [1, 2, 3, 4, 5]);

        let s = Serial::from([1, 2, 3, 4, 5]);
        assert_eq!(<[u8; 5]>::from(s), [1, 2, 3, 4, 5]);

        assert_eq!(<heapless::String<10>>::from(&s).as_str(), "0102030405");

        assert_eq!(Serial::try_from("010203FFFE").unwrap().0, [1, 2, 3, 255, 254]);
    }
}
