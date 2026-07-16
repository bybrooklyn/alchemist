#![no_main]

//! Fuzz `parse_annex_b` against arbitrary byte input.
//! The parser must never panic, regardless of input.

use libfuzzer_sys::fuzz_target;
use rust_h264::nal::parse_annex_b;

fuzz_target!(|data: &[u8]| {
    let _ = parse_annex_b(data);
});
