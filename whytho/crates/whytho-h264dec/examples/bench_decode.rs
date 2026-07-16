use rust_h264::decoder::Decoder;
use rust_h264::nal::parse_annex_b;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let data = std::fs::read(path).unwrap();
    let nals = parse_annex_b(&data);
    let mut decoder = Decoder::new();
    let mut count = 0u32;
    for nal in &nals {
        if let Ok(Some(_f)) = decoder.decode_nal(nal) {
            count += 1;
        }
    }
    if let Some(_f) = decoder.flush() {
        count += 1;
    }
    eprintln!("Decoded {} frames", count);
}
