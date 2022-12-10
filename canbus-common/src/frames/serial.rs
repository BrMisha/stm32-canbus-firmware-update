#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let s = Serial::from([1, 2, 3, 4, 5]);
        assert_eq!(s.0, [1, 2, 3, 4, 5]);

        let s = Serial::from([1, 2, 3, 4, 5]);
        assert_eq!(<[u8; 5]>::from(s), [1, 2, 3, 4, 5]);
    }
}
