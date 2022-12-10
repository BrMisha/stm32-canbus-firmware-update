use crate::frames::serial::Serial;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Data {
    pub serial: Serial,
    pub dyn_id: u8,
}

impl Data {
    pub fn new(serial: Serial, dyn_id: u8) -> Self {
        Self { serial, dyn_id }
    }
}

impl From<[u8; 6]> for Data {
    fn from(value: [u8; 6]) -> Self {
        Self {
            serial: crate::frames::serial::Serial::from(<[u8; 5]>::try_from(&value[0..5]).unwrap()),
            dyn_id: value[5],
        }
    }
}

impl From<Data> for [u8; 6] {
    fn from(d: Data) -> Self {
        let mut tt: [u8; 6] = Default::default();

        let t: &[u8; 5] = &d.serial.into();
        tt[..5].clone_from_slice(t);
        tt[5] = d.dyn_id;

        tt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let s = Data::new(Serial::from([1, 2, 3, 4, 5]), 10);
        assert_eq!(s.serial, Serial::from([1, 2, 3, 4, 5]));
        assert_eq!(s.dyn_id, 10);

        let s = Data::new(Serial::from([1, 2, 3, 0, 5]), 0);
        assert_eq!(s.serial, Serial::from([1, 2, 3, 0, 5]));
        assert_eq!(s.dyn_id, 0);

        let bd = [1, 2, 3, 4, 5, 12];
        let s = Data::from(bd);
        assert_eq!(s.serial, Serial::from([1, 2, 3, 4, 5]));
        assert_eq!(s.dyn_id, 12);

        let s = Data::new(Serial::from([1, 2, 3, 4, 5]), 10);
        assert_eq!(
            {
                let d: [u8; 6] = s.into();
                d
            },
            [1, 2, 3, 4, 5, 10]
        );
    }
}
