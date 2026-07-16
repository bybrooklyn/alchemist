use rust_h264::decoder::Decoder;
use rust_h264::nal::parse_annex_b;

fn main() {
    let data = std::fs::read("/tmp/bars.h264").unwrap();
    let nals = parse_annex_b(&data);

    let mut decoder = Decoder::new();
    for nal in &nals {
        match decoder.decode_nal(nal) {
            Ok(Some(frame)) => {
                println!("Decoded frame: {}x{}", frame.width, frame.height);

                // Compare with expected
                let expected = std::fs::read("/tmp/bars_decoded.yuv").unwrap();
                let mut our_output = Vec::new();
                our_output.extend_from_slice(&frame.y);
                our_output.extend_from_slice(&frame.u);
                our_output.extend_from_slice(&frame.v);

                if our_output == expected {
                    println!("SUCCESS: Output matches expected!");
                } else {
                    println!("MISMATCH: Output differs from expected");
                    println!("\nFirst 16 Y pixels:");
                    println!("  Ours:     {:?}", &frame.y[..16]);
                    println!("  Expected: {:?}", &expected[..16]);
                }
            }
            Ok(None) => {}
            Err(e) => println!("Error: {}", e),
        }
    }
}
