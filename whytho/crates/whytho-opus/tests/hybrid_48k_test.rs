use opus_rs::{Application, OpusDecoder, OpusEncoder};

#[test]
fn test_48k_hybrid_quality() {
    let sample_rate = 48000;
    let frame_size = 960; // 20ms at 48kHz
    let num_frames = 10;
    let total_samples = frame_size * num_frames;

    // Create encoder/decoder in VoIP mode (uses Hybrid for 48kHz)
    let mut encoder = OpusEncoder::new(sample_rate, 1, Application::Voip).unwrap();
    encoder.bitrate_bps = 32000;
    encoder.use_cbr = true;

    let mut decoder = OpusDecoder::new(sample_rate, 1).unwrap();

    // Generate all input samples and collect all output samples
    let input: Vec<f32> = (0..total_samples)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            (f64::sin(2.0 * std::f64::consts::PI * 1000.0 * t) * 0.5) as f32
        })
        .collect();

    let mut output = vec![0.0f32; total_samples];
    for frame in 0..num_frames {
        let s = frame * frame_size;
        let e = s + frame_size;
        let mut encoded = vec![0u8; 512];
        let len = encoder
            .encode(&input[s..e], frame_size, &mut encoded)
            .unwrap();
        encoded.truncate(len);
        decoder
            .decode(&encoded, frame_size, &mut output[s..e])
            .unwrap();
    }

    // The codec introduces a fixed latency (SILK resampling + algorithmic delay).
    // Measure SNR after compensating for this delay via cross-correlation.
    // Without compensation, a perfectly-working codec still shows ~-3 dB SNR for a
    // 1 kHz sine when the delay is ~quarter-period.
    let skip = frame_size * 2; // skip first two frames (codec warm-up)
    let max_search = frame_size / 2;
    let best_delay = (0..max_search)
        .map(|d| {
            let corr: f64 = input[skip..total_samples - d]
                .iter()
                .zip(output[skip + d..].iter())
                .map(|(a, b)| *a as f64 * *b as f64)
                .sum();
            (d, corr)
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(d, _)| d)
        .unwrap_or(0);

    let n = total_samples - skip - best_delay;
    let input_energy: f64 = input[skip..skip + n]
        .iter()
        .map(|x| (*x as f64).powi(2))
        .sum();
    let error_energy: f64 = input[skip..skip + n]
        .iter()
        .zip(output[skip + best_delay..skip + best_delay + n].iter())
        .map(|(a, b)| ((*a - *b) as f64).powi(2))
        .sum();

    let snr = 10.0 * (input_energy / error_energy).log10();
    println!(
        "48kHz Hybrid mode SNR (delay-corrected, delay={} samples): {:.2} dB",
        best_delay, snr
    );

    assert!(
        snr > 10.0,
        "Hybrid mode SNR {:.2} dB is too low (expected >10 dB). \
         SILK+CELT decode is producing distorted output.",
        snr
    );
}
