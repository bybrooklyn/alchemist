#![allow(clippy::doc_lazy_continuation)] // auto-generated docs may have lazy continuation
#![allow(clippy::derivable_impls)] // auto-generated code may have derivable impls
use crate::base::VInt64;
use crate::element::Element;

use bytes::*;

// Auto-generated element types.
include!(concat!(env!("OUT_DIR"), "/generated_types.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uint() {
        let test_pair = [
            (vec![1u8], 1u64),
            (vec![0u8], 0u64),
            (vec![0xFFu8], 255u64),
            (vec![0x01u8, 0], 256u64),
            (vec![0x01u8, 0xFF], 256u64 + 255),
            (vec![0xFFu8, 0xFFu8], 2u64.pow(16) - 1),
            (vec![1, 0, 0], 2u64.pow(16)),
            (vec![1, 0, 0, 0], 2u64.pow(24)),
            (vec![1, 0, 0, 0, 0, 0, 0, 0], 2u64.pow(56)),
            (vec![0xFF; 8], u64::MAX),
        ];
        for (encoded, decoded) in test_pair {
            let v = DocTypeVersion::decode_body(&mut &*encoded).unwrap();
            assert_eq!(v, DocTypeVersion(decoded));

            let mut buf = vec![];
            DocTypeVersion(decoded).encode_body(&mut buf).unwrap();
            assert_eq!(buf, encoded);
        }
    }
    #[test]
    fn test_sint() {
        assert_eq!(-2i64.pow(15), -32768);

        let positive = |n: u32| 2i64.pow((n * 8) - 1) - 1;
        let negative = |n: u32| -2i64.pow((n * 8) - 1);

        let test_pair = [
            (vec![0u8], 0i64),
            (vec![1u8], 1i64),
            (vec![0xFF], -1i64),
            (vec![0x2A], 42),
            (vec![0xD6], -42),
            (vec![0x03, 0xE8], 1000),
            (vec![0xFC, 0x18], -1000),
            (vec![0x7F], 127),                                 // 2^7 - 1
            (vec![0x80], -128),                                // -2^7
            (vec![0x7F, 0xFF], positive(2)),                   // 2^15 - 1
            (vec![0x80, 0x00], negative(2)),                   // -2^15
            (vec![0x7F, 0xFF, 0xFF], positive(3)),             // 2^23 - 1
            (vec![0x80, 0x00, 0x00], negative(3)),             // -2^23
            (vec![0x7F, 0xFF, 0xFF, 0xFF], positive(4)),       // 2^31 - 1
            (vec![0x80, 0x00, 0x00, 0x00], negative(4)),       // -2^31
            (vec![0x7F, 0xFF, 0xFF, 0xFF, 0xFF], positive(5)), // 2^39 -1
            (vec![0x80, 0x00, 0x00, 0x00, 0x00], negative(5)), // -2^39
            (
                vec![0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                i64::MAX,
            ),
            (
                vec![0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                i64::MIN,
            ),
        ];
        for (encoded, decoded) in test_pair {
            let v = ReferenceBlock::decode_body(&mut &*encoded).unwrap();
            assert_eq!(v, ReferenceBlock(decoded));

            let mut buf = vec![];
            ReferenceBlock(decoded).encode_body(&mut buf).unwrap();
            assert_eq!(buf, encoded);
        }
    }

    #[test]
    fn test_float() {
        let test_pair = [
            0f64,
            -1.0,
            1.0,
            f32::MIN_POSITIVE as f64,
            f32::MIN as f64,
            f32::MAX as f64,
            f64::MIN_POSITIVE,
            f64::MIN,
            f64::MAX,
        ]
        .iter()
        .map(|&v| (v.to_be_bytes().to_vec(), v));

        for (encoded, decoded) in test_pair {
            let v = Duration::decode_body(&mut &*encoded).unwrap();
            assert_eq!(v, Duration(decoded));

            let mut buf = vec![];
            Duration(decoded).encode_body(&mut buf).unwrap();
            let new_v = Duration::decode_body(&mut &*buf).unwrap();
            assert_eq!(new_v, Duration(decoded));
        }
    }

    #[test]
    fn test_text() {
        let test_pair = [
            (vec![], ""),
            (vec![b'h', b'e', b'y'], "hey"),
            (vec![b'h', b'e', b'y', 0, b'a'], "hey"),
            ("testing utf8 ✓".as_bytes().to_vec(), "testing utf8 ✓"),
            ("こんにちは".as_bytes().to_vec(), "こんにちは"),
            (vec![b'h', b'e', b'y', 0, b'w'], "hey"),
        ];

        for (input, output) in test_pair {
            let v = SegmentFilename::decode_body(&mut &*input).unwrap();
            assert_eq!(v, SegmentFilename(output.to_string()));

            let mut encoded = vec![];
            SegmentFilename(output.to_string())
                .encode_body(&mut encoded)
                .unwrap();

            assert_eq!(encoded[encoded.len() - 1], 0); // should be null-terminated
            let input_zero_pos = input.iter().position(|&b| b == 0).unwrap_or(input.len());
            assert_eq!(encoded[..input_zero_pos], input[..input_zero_pos]); // should encode

            let new_decoded = SegmentFilename::decode_body(&mut &*encoded).unwrap();
            assert_eq!(new_decoded, SegmentFilename(output.to_string()));
        }
    }

    #[test]
    fn test_bin() {
        let test_pair = [
            (vec![], vec![]),
            (vec![1, 2, 3], vec![1u8, 2, 3]),
            ((0..=255).collect(), (0..=255).collect()),
        ];

        for (encoded, decoded) in test_pair {
            let v = SeekId::decode_body(&mut &*encoded).unwrap();
            assert_eq!(v, SeekId(Bytes::from(decoded.clone())));

            let mut buf = vec![];
            SeekId(Bytes::from(decoded.clone()))
                .encode_body(&mut buf)
                .unwrap();
            assert_eq!(buf, encoded);
        }
    }

    #[test]
    fn test_date() {
        let test_cases = [0i64, 1, -1, i64::MIN, i64::MAX];

        for n in test_cases {
            let date = DateUtc::decode_body(&mut &n.to_be_bytes()[..]).unwrap();
            assert_eq!(date, DateUtc(n));

            let mut buf = vec![];
            date.encode_body(&mut buf).unwrap();
            assert_eq!(buf, n.to_be_bytes());
        }
    }
}
