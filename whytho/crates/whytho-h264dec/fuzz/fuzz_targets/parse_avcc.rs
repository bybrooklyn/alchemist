#![no_main]

//! Fuzz `parse_avcc` against arbitrary byte input with various length sizes.
//! The parser must never panic.

use libfuzzer_sys::fuzz_target;
use rust_h264::nal::parse_avcc;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    // Use the first byte to pick a length size (1, 2, or 4)
    let length_size = match data[0] & 0x03 {
        0 => 1,
        1 => 2,
        _ => 4,
    };
    let _ = parse_avcc(&data[1..], length_size);
});
