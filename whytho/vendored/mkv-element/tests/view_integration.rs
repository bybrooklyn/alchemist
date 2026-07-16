#![cfg(feature = "utils")]

use mkv_element::io::blocking_impl::{WriteElement, WriteTo};
use mkv_element::prelude::*;
use mkv_element::view::MatroskaView;
use std::io::Cursor;

/// Helper function to create a standard EBML header for Matroska
fn ebml() -> Ebml {
    Ebml {
        crc32: None,
        ebml_version: Some(EbmlVersion(1)),
        ebml_read_version: Some(EbmlReadVersion(1)),
        ebml_max_id_length: EbmlMaxIdLength(4),
        ebml_max_size_length: EbmlMaxSizeLength(8),
        doc_type: Some(DocType("matroska".to_string())),
        doc_type_version: Some(DocTypeVersion(4)),
        doc_type_read_version: Some(DocTypeReadVersion(2)),
        void: None,
    }
}

/// Helper function to create the first test segment with basic info
fn segment1() -> Segment {
    let info = Info {
        timestamp_scale: TimestampScale(1000000), // 1ms per tick (default)
        muxing_app: MuxingApp("mkv-element".to_string()),
        writing_app: WritingApp("integration-test".to_string()),
        title: Some(Title("Test Segment 1".to_string())),
        duration: Some(Duration(30000.0)), // 30 seconds
        ..Default::default()
    };

    // Create a simple video track
    let video_track = TrackEntry {
        track_number: TrackNumber(1),
        track_uid: TrackUid(1234567890),
        track_type: TrackType(1), // Video
        codec_id: CodecId("V_VP9".to_string()),
        name: Some(Name("Video Track".to_string())),
        codec_name: Some(CodecName("VP9".to_string())),
        video: Some(Video {
            pixel_width: PixelWidth(1920),
            pixel_height: PixelHeight(1080),
            ..Default::default()
        }),
        ..Default::default()
    };

    let tracks = Tracks {
        track_entry: vec![video_track],
        ..Default::default()
    };

    // Create a simple cluster with dummy data
    let cluster = Cluster {
        timestamp: Timestamp(0),
        blocks: vec![], // Empty for simplicity
        ..Default::default()
    };

    Segment {
        crc32: None,
        void: None,
        seek_head: vec![],
        info,
        cluster: vec![cluster],
        tracks: Some(tracks),
        cues: None,
        attachments: None,
        chapters: None,
        tags: vec![],
    }
}

fn segment_without_clusters() -> Segment {
    let info = Info {
        timestamp_scale: TimestampScale(1000000),
        muxing_app: MuxingApp("mkv-element".to_string()),
        writing_app: WritingApp("integration-test".to_string()),
        title: Some(Title("No Clusters Segment".to_string())),
        ..Default::default()
    };

    // Create a simple audio track
    let audio_track = TrackEntry {
        track_number: TrackNumber(1),
        track_uid: TrackUid(9876543210),
        track_type: TrackType(2), // Audio
        codec_id: CodecId("A_OPUS".to_string()),
        name: Some(Name("Audio Track".to_string())),
        codec_name: Some(CodecName("Opus".to_string())),
        audio: Some(Audio {
            sampling_frequency: SamplingFrequency(48000.0),
            channels: Channels(2),
            ..Default::default()
        }),
        ..Default::default()
    };

    let tracks = Tracks {
        track_entry: vec![audio_track],
        ..Default::default()
    };

    Segment {
        crc32: None,
        void: None,
        seek_head: vec![],
        info,
        cluster: vec![], // No clusters
        tracks: Some(tracks),
        cues: None,
        attachments: None,
        chapters: None,
        tags: vec![],
    }
}

#[test]
fn test_basic_matroska_view() {
    // Create a Matroska file with EBML header and a basic segment
    let ebml_header = ebml();
    let segment = segment1();

    // Serialize to bytes
    let mut buffer = Vec::new();
    ebml_header.write_to(&mut buffer).unwrap();
    segment.write_to(&mut buffer).unwrap();

    let mut cursor = Cursor::new(&buffer);
    let view = MatroskaView::new(&mut cursor).unwrap();
    assert_eq!(view.ebml.doc_type.as_deref(), Some("matroska"));
    assert_eq!(view.segments.len(), 1);
    let segment_view = &view.segments[0];
    assert_eq!(segment_view.info.title.as_deref(), Some("Test Segment 1"));
    assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
    assert_ne!(segment_view.first_cluster_position, 0);
}

#[test]
fn test_segment_without_clusters() {
    // Create a Matroska file with EBML header and a segment without clusters
    let ebml_header = ebml();
    let segment = segment_without_clusters();

    // Serialize to bytes
    let mut buffer = Vec::new();
    ebml_header.write_to(&mut buffer).unwrap();
    segment.write_to(&mut buffer).unwrap();

    let mut cursor = Cursor::new(&buffer);
    let view = MatroskaView::new(&mut cursor).unwrap();
    assert_eq!(view.ebml.doc_type.as_deref(), Some("matroska"));
    assert_eq!(view.segments.len(), 1);
    let segment_view = &view.segments[0];
    assert_eq!(
        segment_view.info.title.as_deref(),
        Some("No Clusters Segment")
    );
    assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
    assert_eq!(segment_view.first_cluster_position, 0); // No clusters present
}

#[test]
fn test_multiple_segments() {
    // Create a Matroska file with EBML header and multiple segments
    let ebml_header = ebml();
    let segment1 = segment1();
    let segment2 = segment1.clone();
    let segment3 = segment1.clone();
    let segment4 = segment_without_clusters();

    // Serialize to bytes
    let mut buffer = Vec::new();
    ebml_header.write_to(&mut buffer).unwrap();
    segment1.write_to(&mut buffer).unwrap();
    segment2.write_to(&mut buffer).unwrap();
    segment3.write_to(&mut buffer).unwrap();
    segment4.write_to(&mut buffer).unwrap();

    let mut cursor = Cursor::new(&buffer);
    let view = MatroskaView::new(&mut cursor).unwrap();
    assert_eq!(view.ebml.doc_type.as_deref(), Some("matroska"));
    assert_eq!(view.segments.len(), 4, "should have 4 segments");

    for (i, segment_view) in view.segments.iter().enumerate() {
        if i < 3 {
            assert_eq!(segment_view.info.title.as_deref(), Some("Test Segment 1"));
            assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
            assert_ne!(segment_view.first_cluster_position, 0);
        } else {
            assert_eq!(
                segment_view.info.title.as_deref(),
                Some("No Clusters Segment")
            );
            assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
            assert_eq!(segment_view.first_cluster_position, 0); // No clusters present
        }
    }
}

#[test]
fn test_unsize_segment() {
    let ebml_header = ebml();

    let segment_header = Header {
        id: Segment::ID,
        size: VInt64::new_unknown(),
    };
    let segment = segment1();
    let mut buffer = Vec::new();
    ebml_header.write_to(&mut buffer).unwrap();
    segment.write_element(&segment_header, &mut buffer).unwrap();
    let mut cursor = Cursor::new(&buffer);
    let view = MatroskaView::new(&mut cursor).unwrap();
    assert_eq!(view.ebml.doc_type.as_deref(), Some("matroska"));
    assert_eq!(view.segments.len(), 1);
    let segment_view = &view.segments[0];
    assert_eq!(segment_view.info.title.as_deref(), Some("Test Segment 1"));
    assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
    assert_ne!(segment_view.first_cluster_position, 0);
}

#[cfg(feature = "tokio")]
mod async_tests {
    use super::*;
    use mkv_element::io::tokio_impl::{AsyncWriteElement, AsyncWriteTo};

    #[tokio::test]
    async fn test_basic_matroska_view_async() {
        // Create a Matroska file with EBML header and a basic segment
        let ebml_header = ebml();
        let segment = segment1();

        // Serialize to bytes
        let mut buffer = Vec::new();
        ebml_header.async_write_to(&mut buffer).await.unwrap();
        segment.async_write_to(&mut buffer).await.unwrap();

        let mut cursor = Cursor::new(&buffer);
        let view = MatroskaView::new_async(&mut cursor).await.unwrap();
        assert_eq!(view.ebml.doc_type.as_deref(), Some("matroska"));
        assert_eq!(view.segments.len(), 1);
        let segment_view = &view.segments[0];
        assert_eq!(segment_view.info.title.as_deref(), Some("Test Segment 1"));
        assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
        assert_ne!(segment_view.first_cluster_position, 0);
    }

    #[tokio::test]
    async fn test_segment_without_clusters_async() {
        // Create a Matroska file with EBML header and a segment without clusters
        let ebml_header = ebml();
        let segment = segment_without_clusters();

        // Serialize to bytes
        let mut buffer = Vec::new();
        ebml_header.async_write_to(&mut buffer).await.unwrap();
        segment.async_write_to(&mut buffer).await.unwrap();

        let mut cursor = Cursor::new(&buffer);
        let view = MatroskaView::new_async(&mut cursor).await.unwrap();
        assert_eq!(view.ebml.doc_type.as_deref(), Some("matroska"));
        assert_eq!(view.segments.len(), 1);
        let segment_view = &view.segments[0];
        assert_eq!(
            segment_view.info.title.as_deref(),
            Some("No Clusters Segment")
        );
        assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
        assert_eq!(segment_view.first_cluster_position, 0); // No clusters present
    }

    #[tokio::test]
    async fn test_multiple_segments_async() {
        // Create a Matroska file with EBML header and multiple segments
        let ebml_header = ebml();
        let segment1 = segment1();
        let segment2 = segment1.clone();
        let segment3 = segment1.clone();
        let segment4 = segment_without_clusters();

        // Serialize to bytes
        let mut buffer = Vec::new();
        ebml_header.async_write_to(&mut buffer).await.unwrap();
        segment1.async_write_to(&mut buffer).await.unwrap();
        segment2.async_write_to(&mut buffer).await.unwrap();
        segment3.async_write_to(&mut buffer).await.unwrap();
        segment4.async_write_to(&mut buffer).await.unwrap();

        let mut cursor = Cursor::new(&buffer);
        let view = MatroskaView::new_async(&mut cursor).await.unwrap();
        assert_eq!(view.ebml.doc_type.as_deref(), Some("matroska"));
        assert_eq!(view.segments.len(), 4, "should have 4 segments");

        for (i, segment_view) in view.segments.iter().enumerate() {
            if i < 3 {
                assert_eq!(segment_view.info.title.as_deref(), Some("Test Segment 1"));
                assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
                assert_ne!(segment_view.first_cluster_position, 0);
            } else {
                assert_eq!(
                    segment_view.info.title.as_deref(),
                    Some("No Clusters Segment")
                );
                assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
                assert_eq!(segment_view.first_cluster_position, 0); // No clusters present
            }
        }
    }

    #[tokio::test]
    async fn test_unsize_segment_async() {
        let ebml_header = ebml();

        let segment_header = Header {
            id: Segment::ID,
            size: VInt64::new_unknown(),
        };
        let segment = segment1();
        let mut buffer = Vec::new();
        ebml_header.async_write_to(&mut buffer).await.unwrap();
        segment
            .async_write_element(&segment_header, &mut buffer)
            .await
            .unwrap();
        let mut cursor = Cursor::new(&buffer);
        let view = MatroskaView::new_async(&mut cursor).await.unwrap();
        assert_eq!(view.ebml.doc_type.as_deref(), Some("matroska"));
        assert_eq!(view.segments.len(), 1);
        let segment_view = &view.segments[0];
        assert_eq!(segment_view.info.title.as_deref(), Some("Test Segment 1"));
        assert_eq!(segment_view.tracks.as_ref().unwrap().track_entry.len(), 1);
        assert_ne!(segment_view.first_cluster_position, 0);
    }
}
