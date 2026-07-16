#![no_main]

//! Fuzz `parse_avcc_config` (the MP4 `avcC` box parser) against arbitrary
//! byte input. The parser must return either Ok or Err — never panic.

use libfuzzer_sys::fuzz_target;
use rust_h264::nal::parse_avcc_config;

fuzz_target!(|data: &[u8]| {
    let _ = parse_avcc_config(data);
});
