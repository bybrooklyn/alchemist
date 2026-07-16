use rust_h264::decoder::Decoder;
use rust_h264::nal::parse_annex_b;

fn main() {
    let data = std::fs::read("/tmp/solid_gray.h264").unwrap();
    let nals = parse_annex_b(&data);

    println!("NALs found: {}", nals.len());
    for (i, nal) in nals.iter().enumerate() {
        println!("  NAL {}: {:?}", i, nal.nal_unit_type);
    }

    let mut decoder = Decoder::new();
    for nal in &nals {
        match decoder.decode_nal(nal) {
            Ok(Some(frame)) => {
                println!("Decoded frame: {}x{}", frame.width, frame.height);
                println!("First 16 Y pixels: {:?}", &frame.y[..16]);

                // Compare with expected
                let expected = std::fs::read("/tmp/solid_gray.yuv").unwrap();
                let mut our_output = Vec::new();
                our_output.extend_from_slice(&frame.y);
                our_output.extend_from_slice(&frame.u);
                our_output.extend_from_slice(&frame.v);

                if our_output == expected {
                    println!("SUCCESS: Output matches expected!");
                } else {
                    println!("MISMATCH: Output differs from expected");
                    // Show differences
                    for i in 0..16.min(our_output.len()) {
                        if our_output[i] != expected[i] {
                            println!(
                                "  Y[{}]: ours={}, expected={}",
                                i, our_output[i], expected[i]
                            );
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(e) => println!("Error: {}", e),
        }
    }
}
