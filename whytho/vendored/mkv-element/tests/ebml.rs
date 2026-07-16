use mkv_element::prelude::*;

#[test]
fn read_ebml() {
    use mkv_element::io::blocking_impl::*;
    let ebml_hex = [
        0x1a, 0x45, 0xDF, 0xA3, 0x93, 0x42, 0x82, 0x88, 0x6D, 0x61, 0x74, 0x72, 0x6F, 0x73, 0x6B,
        0x61, 0x42, 0x87, 0x81, 0x01, 0x42, 0x85, 0x81, 0x01,
    ];
    let mut ebml_hex = std::io::Cursor::new(ebml_hex);
    let ebml = Ebml::read_from(&mut ebml_hex).unwrap();
    let ebml_expected = Ebml {
        crc32: None,
        ebml_version: None,
        ebml_read_version: None,
        ebml_max_id_length: EbmlMaxIdLength(4),
        ebml_max_size_length: EbmlMaxSizeLength(8),
        doc_type: Some(DocType("matroska".to_string())),
        doc_type_version: Some(DocTypeVersion(1)),
        doc_type_read_version: Some(DocTypeReadVersion(1)),
        void: None,
    };
    assert_eq!(ebml, ebml_expected);
}

#[test]
fn write_ebml() {
    use mkv_element::io::blocking_impl::*;
    let ebml = Ebml {
        crc32: None,
        ebml_version: None,
        ebml_read_version: None,
        ebml_max_id_length: EbmlMaxIdLength(4),
        ebml_max_size_length: EbmlMaxSizeLength(8),
        doc_type: Some(DocType("matroska".to_string())),
        doc_type_version: Some(DocTypeVersion(1)),
        doc_type_read_version: Some(DocTypeReadVersion(1)),
        void: None,
    };
    let mut ebml_buf = Vec::new();
    ebml.write_to(&mut ebml_buf).unwrap();
    let ebml_read = Ebml::read_from(&mut &ebml_buf[..]).unwrap();
    assert_eq!(ebml, ebml_read);
}

#[test]
fn test_signed_integer_preserve_sign_bit() {
    // Encode
    let dp = DiscardPadding(14833333_i64);
    let mut buf = Vec::new();
    dp.encode_body(&mut buf).unwrap();
    // Decode
    let mut b: &[u8] = &buf[..];
    let decoded = DiscardPadding::decode_body(&mut b).unwrap();
    assert_eq!(dp, decoded);
    // Also test negative for regressions
    let dp_neg = DiscardPadding(-14833333_i64);
    let mut buf_neg = Vec::new();
    dp_neg.encode_body(&mut buf_neg).unwrap();
    let mut b_neg: &[u8] = &buf_neg[..];
    let decoded_neg = DiscardPadding::decode_body(&mut b_neg).unwrap();
    assert_eq!(dp_neg, decoded_neg);
}

#[cfg(feature = "tokio")]
mod tokio_tests {
    use mkv_element::io::tokio_impl::*;
    use mkv_element::prelude::*;

    #[tokio::test]
    async fn read_ebml_tokio() {
        let ebml_hex = [
            0x1a, 0x45, 0xDF, 0xA3, 0x93, 0x42, 0x82, 0x88, 0x6D, 0x61, 0x74, 0x72, 0x6F, 0x73,
            0x6B, 0x61, 0x42, 0x87, 0x81, 0x01, 0x42, 0x85, 0x81, 0x01,
        ];
        let mut ebml_hex = std::io::Cursor::new(ebml_hex);
        let ebml = Ebml::async_read_from(&mut ebml_hex).await.unwrap();
        let ebml_expected = Ebml {
            crc32: None,
            ebml_version: None,
            ebml_read_version: None,
            ebml_max_id_length: EbmlMaxIdLength(4),
            ebml_max_size_length: EbmlMaxSizeLength(8),
            doc_type: Some(DocType("matroska".to_string())),
            doc_type_version: Some(DocTypeVersion(1)),
            doc_type_read_version: Some(DocTypeReadVersion(1)),
            void: None,
        };
        assert_eq!(ebml, ebml_expected);
    }

    #[tokio::test]
    async fn write_ebml_tokio() {
        let ebml = Ebml {
            crc32: None,
            ebml_version: None,
            ebml_read_version: None,
            ebml_max_id_length: EbmlMaxIdLength(4),
            ebml_max_size_length: EbmlMaxSizeLength(8),
            doc_type: Some(DocType("matroska".to_string())),
            doc_type_version: Some(DocTypeVersion(1)),
            doc_type_read_version: Some(DocTypeReadVersion(1)),
            void: None,
        };
        let mut ebml_buf = Vec::new();
        ebml.async_write_to(&mut ebml_buf).await.unwrap();
        let ebml_read = Ebml::async_read_from(&mut &ebml_buf[..]).await.unwrap();
        assert_eq!(ebml, ebml_read);
    }
}
