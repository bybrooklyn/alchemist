#![no_main]

//! End-to-end fuzz: parse arbitrary input as Annex B and feed every NAL to
//! the decoder. The decoder must never panic — it should return `Err` on
//! malformed input but stay alive.

use libfuzzer_sys::fuzz_target;
use rust_h264::decoder::Decoder;
use rust_h264::nal::parse_annex_b;

fuzz_target!(|data: &[u8]| {
    let nals = parse_annex_b(data);
    let mut decoder = Decoder::new();
    for nal in &nals {
        // Ignore errors — the goal is to verify no panics, not to validate
        // success.
        let _ = decoder.decode_nal(nal);
    }
    let _ = decoder.flush();
});
