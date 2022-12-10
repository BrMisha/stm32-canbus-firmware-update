#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct FirmwareUpdate<const PAGE_SIZE: usize, const PART_SIZE: usize, const BUFF_SIZE: usize> {
    buff: arrayvec::ArrayVec<u8, BUFF_SIZE>,
    loaded_parts_count: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PutPartError {
    LessOfMinPart(usize),
    MoreOfMaxPart(usize),
    NotEnoughSpace,
}

impl<const PAGE_SIZE: usize, const PART_SIZE: usize, const BUFF_SIZE: usize>
    FirmwareUpdate<PAGE_SIZE, PART_SIZE, BUFF_SIZE>
{
    pub fn new() -> Option<Self> {
        if BUFF_SIZE != PAGE_SIZE + PART_SIZE {
            return None;
        }
        Some(Self::default())
    }

    pub fn reset(&mut self) {
        *self = Default::default()
    }

    pub fn put_part(
        &mut self,
        part: [u8; PART_SIZE],
        part_number: usize,
    ) -> Result<(), PutPartError> {
        if part_number == 0 {
            self.reset();
        }

        if part_number < self.loaded_parts_count {
            let size_to_remove = (self.loaded_parts_count - part_number) * PART_SIZE;
            if size_to_remove > self.len() {
                return Err(PutPartError::LessOfMinPart(
                    self.loaded_parts_count - (self.len() / PART_SIZE),
                ));
            }

            self.loaded_parts_count -= size_to_remove / PART_SIZE;
            self.buff.drain((self.buff.len() - size_to_remove)..);
        } else if part_number > self.loaded_parts_count {
            return Err(PutPartError::MoreOfMaxPart(self.loaded_parts_count));
        }

        if part.len() > self.buff.remaining_capacity() {
            return Err(PutPartError::NotEnoughSpace);
        }

        self.loaded_parts_count += part.len() / PART_SIZE;

        self.buff.extend(part);

        Ok(())
    }

    pub fn page_is_ready(&self) -> bool {
        self.buff.len() >= PAGE_SIZE
    }

    pub fn get_page(&self) -> Option<(&[u8; PAGE_SIZE], usize)> {
        if self.page_is_ready() == false {
            return None;
        }
        (<&[u8; PAGE_SIZE]>::try_from(&self.buff[..PAGE_SIZE]))
            .map(|v| (v, self.loaded_parts_count * PART_SIZE / PAGE_SIZE - 1))
            .ok()
    }

    pub fn remove_page(&mut self) -> bool {
        if self.page_is_ready() == false {
            return false;
        }
        self.buff.drain(..PAGE_SIZE);

        true
    }

    pub fn len(&self) -> usize {
        self.buff.len()
    }

    pub fn loaded_parts_count(&self) -> usize {
        self.loaded_parts_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen_array<const SIZE: usize>() -> [u8; SIZE] {
        let mut arr = [0_u8; SIZE];
        arr.iter_mut().for_each(|v| *v = rand::random());
        arr
    }

    fn test_mod<const PAGE_SIZE: usize, const PART_SIZE: usize, const SMALL_SIZE: usize>(
        test_data: &[u8],
    ) {
        let mut obj = FirmwareUpdate::<PAGE_SIZE, PART_SIZE, SMALL_SIZE>::new().unwrap();

        // put 5
        {
            let mut r = [0_u8; PART_SIZE];
            for v in &mut r {
                *v = 10;
            }

            assert_eq!(obj.put_part(r, 0), Ok(()));
            assert_eq!(obj.len(), 5);
            assert_eq!(*obj.buff, r);
        }

        obj.reset();
        assert_eq!(
            (obj.len(), obj.buff.len(), obj.loaded_parts_count),
            (0, 0, 0)
        );

        let mut part_number = 0usize;
        let mut estimated_page = 0usize;
        for part in test_data.chunks(PART_SIZE).enumerate() {
            let p = <&[u8; PART_SIZE]>::try_from(part.1).unwrap();
            assert_eq!(
                obj.put_part(*p, part_number),
                Ok(()),
                "chunk {}, part {:?}, len {}",
                part.0,
                p,
                obj.len()
            );
            part_number += 1;

            if obj.len() >= PAGE_SIZE {
                let len = obj.len();
                let page = obj.get_page().unwrap();
                assert_eq!(page.0.len(), PAGE_SIZE);
                assert_eq!(page.1, estimated_page);
                assert_eq!(obj.remove_page(), true);
                assert_eq!(obj.len(), len - PAGE_SIZE);
                estimated_page += 1;
            }
        }

        obj.reset();
        assert_eq!(
            (obj.len(), obj.buff.len(), obj.loaded_parts_count),
            (0, 0, 0)
        );
    }

    #[test]
    fn test() {
        test_mod::<1024, 5, { 1024 + 5 }>(&gen_array::<5>());
        test_mod::<1024, 5, { 1024 + 5 }>(&gen_array::<50>());
        test_mod::<1024, 5, { 1024 + 5 }>(&gen_array::<500>());
        test_mod::<1024, 5, { 1024 + 5 }>(&gen_array::<5000>());
        test_mod::<1024, 5, { 1024 + 5 }>(&gen_array::<50000>());
        test_mod::<1024, 5, { 1024 + 5 }>(&gen_array::<500000>());
    }

    #[test]
    fn test2() {
        let test_data = gen_array::<5056>();
        let mut obj = FirmwareUpdate::<16, 5, { 16 + 5 }>::new().unwrap();

        obj.put_part(
            *<&[u8; 5]>::try_from(&test_data[..5]).unwrap(),
            0,
        )
        .unwrap();
        assert_eq!(obj.get_page(), None);
        obj.put_part(
            *<&[u8; 5]>::try_from(&test_data[5..10]).unwrap(),
            1,
        )
        .unwrap();
        assert_eq!(obj.get_page(), None);
        obj.put_part(
            *<&[u8; 5]>::try_from(&test_data[10..15]).unwrap(),
            2,
        )
        .unwrap();
        assert_eq!(obj.get_page(), None);
        obj.put_part(
            *<&[u8; 5]>::try_from(&test_data[15..20]).unwrap(),
            3,
        )
        .unwrap();
        // not enough space
        assert_eq!(
            obj.put_part(
                *<&[u8; 5]>::try_from(&test_data[20..25]).unwrap(),
                4
            ),
            Err(PutPartError::NotEnoughSpace)
        );

        assert_eq!(
            obj.get_page(),
            Some((&<[u8; 16]>::try_from(&test_data[..16]).unwrap(), 0))
        );
        obj.remove_page();
        assert_eq!(obj.len(), 4);

        assert_eq!(
            obj.put_part(
                *<&[u8; 5]>::try_from(&test_data[20..25]).unwrap(),
                4
            ),
            Ok(())
        );
        assert_eq!(
            obj.put_part(
                *<&[u8; 5]>::try_from(&test_data[25..30]).unwrap(),
                5
            ),
            Ok(())
        );

        assert_eq!(
            obj.put_part(
                *<&[u8; 5]>::try_from(&test_data[30..35]).unwrap(),
                3
            ),
            Err(PutPartError::LessOfMinPart(4))
        );
        assert_eq!(
            obj.put_part(
                *<&[u8; 5]>::try_from(&test_data[30..35]).unwrap(),
                7
            ),
            Err(PutPartError::MoreOfMaxPart(6))
        );

        // put again
        assert_eq!(
            obj.put_part(
                *<&[u8; 5]>::try_from(&test_data[20..25]).unwrap(),
                4
            ),
            Ok(())
        );
        assert_eq!(
            obj.put_part(
                *<&[u8; 5]>::try_from(&test_data[25..30]).unwrap(),
                5
            ),
            Ok(())
        );
        assert_eq!(
            obj.put_part(
                *<&[u8; 5]>::try_from(&test_data[30..35]).unwrap(),
                6
            ),
            Ok(())
        );

        assert_eq!(
            obj.get_page(),
            Some((&<[u8; 16]>::try_from(&test_data[16..32]).unwrap(), 1))
        );
        obj.remove_page();
        assert_eq!(obj.len(), 3);
    }
}
