pub fn get(location: u32) -> Option<(canbus_common::frames::version::Version, &'static [u8])> {
    let flash_size = u32::from_be_bytes(*unsafe { &*(location as *const [u8; 4]) }); // without len and crc
    // definitely to much
    if flash_size > 512*1024 {
        return None;
    }

    let version = canbus_common::frames::version::Version::from(*unsafe {
        &*((location + 4) as *const [u8; 8])
    });

    // len+version+flash
    let flash_data = unsafe {
        core::slice::from_raw_parts(&*(location as *const u8), (flash_size + 4) as usize)
    };

    let crc = u32::from_be_bytes(*unsafe {
        &*((location + flash_data.len() as u32) as *const [u8; 4])
    });

    if crc32c_hw::compute(flash_data) == crc {
        return Some((version, &flash_data[12..(flash_data.len()-4)]));
    }

    None
}