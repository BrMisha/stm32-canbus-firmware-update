//! `cargo` "language".
//!
//! ```cargo
//! [dependencies]
//! time = "0.1.25"
//! canbus-common = { path = "../canbus-common" }
//! ```

use std::env;

fn main() {
    let file_path = env::args().nth(1).unwrap();
    let file = std::fs::read(file_path.clone()).unwrap();

    let version = <[u8; 8]>::from(canbus_common::frames::version::Version {
        major: 1,
        minor: 2,
        path: 3,
        build: 4,
    });

    let mut data = Vec::<u8>::new();
    //println!("dd {:?} {}", ((version.len() + file.len()) as u32).to_be_bytes(), ((version.len() + file.len()) as u32));
    data.extend(((version.len() + file.len()) as u32).to_be_bytes()); // add len
    data.extend(version);
    data.extend(&file);
    std::fs::write(file_path, data).unwrap();
}
