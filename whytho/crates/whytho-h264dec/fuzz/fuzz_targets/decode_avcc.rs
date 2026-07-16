#![no_main]

//! End-to-end fuzz for the AVCC code path: parse the input as an `avcC`
//! configuration record and treat the rest as a single AVCC sample. Feeds
//! the SPS/PPS NALs and the sample NALs to the decoder.

use libfuzzer_sys::fuzz_target;
use rust_h264::decoder::Decoder;
use rust_h264::nal::{parse_avcc, parse_avcc_config};

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    // First byte: split point between avcC box and sample data
    let split = (data[0] as usize).min(data.len() - 1);
    let avcc_box = &data[1..1 + split];
    let sample_data = &data[1 + split..];

    let cfg = match parse_avcc_config(avcc_box) {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut decoder = Decoder::new();
    for nal in cfg.sps_nals.iter().chain(cfg.pps_nals.iter()) {
        let _ = decoder.decode_nal(nal);
    }
    for nal in parse_avcc(sample_data, cfg.length_size) {
        let _ = decoder.decode_nal(&nal);
    }
    let _ = decoder.flush();
});
