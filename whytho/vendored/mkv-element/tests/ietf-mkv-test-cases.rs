use core::panic;
use std::io::{Read, Seek, sink};

use mkv_element::ClusterBlock;
use mkv_element::io::blocking_impl::*;
use mkv_element::prelude::*;

// This file is the absolute minimum a compliant player should be able to handle.
// The sample comes from the Big Buck Bunny open project.
// It contains MPEG4.2 (DivX) video, (854x480) MP3 audio, uses only SimpleBlock (matroska DocType v2)
#[test]
#[ignore = "this test requires the matroska-test-files submodule"]
fn ietf_test_1() {
    let mut file = std::fs::File::open("matroska-test-files/test_files/test1.mkv").unwrap();
    let _ebml_head = Ebml::read_from(&mut file).unwrap();
    let segment = Segment::read_from(&mut file).unwrap();
    let tags = segment.tags.first().unwrap();
    let tag = tags.tag.first().unwrap();
    let target_tag = tag.targets.target_type_value;
    assert_eq!(*target_tag, 50);
    let title = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "TITLE")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(title, Some(Some("Big Buck Bunny - test 1")));
    let date_released = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "DATE_RELEASED")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(date_released, Some(Some("2010")));
    let comment = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "COMMENT")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(
        comment,
        Some(Some(
            "Matroska Validation File1, basic MPEG4.2 and MP3 with only SimpleBlock"
        ))
    );

    // It contains MPEG4.2 (DivX) video, (854x480) MP3 audio, uses only SimpleBlock (matroska DocType v2)
    assert!(
        segment.cluster.iter().all(|c| c
            .blocks
            .iter()
            .all(|b| matches!(b, ClusterBlock::Simple(_)))),
        "All clusters should use SimpleBlock only"
    );

    let tracks = segment.tracks.as_ref().unwrap();
    let video_track = tracks.track_entry.iter().find(|t| *t.track_type == 1);
    assert!(video_track.is_some());
    let video_track = video_track.unwrap();

    // Microsoft (TM) Video Codec Manager (VCM)
    assert_eq!(&*video_track.codec_id, "V_MS/VFW/FOURCC");
    assert_eq!(
        video_track.video.as_ref().map(|v| *v.pixel_width),
        Some(854)
    );
    assert_eq!(
        video_track.video.as_ref().map(|v| *v.pixel_height),
        Some(480)
    );
    let audio_track = tracks.track_entry.iter().find(|t| *t.track_type == 2);
    assert!(audio_track.is_some());
    let audio_track = audio_track.unwrap();
    assert_eq!(&*audio_track.codec_id, "A_MPEG/L3");
    assert_eq!(audio_track.audio.as_ref().map(|a| *a.channels), Some(2));
}

// This file has different features that need to be looked at carefully.
// The main one is the global TimecodeScale in the SegmentInfo is set to 100,000 rather than the default 1,000,000.
// That value affects the values of the file Duration in the Segment and the Clusters Timecode.
// The aspect ratio has also been stretched artificially to represent a 2.35 movie (from the original 16:9 aspect ratio).
// This file also contains CRC-32 values in the EBML header, the MetaSeek, the Segment Info, the Tracks and the Tags and PrevSize/Position in the Clusters for better error recovery.
// It contains H264 (1024x576 pixels), and stereo AAC. The source material is taken from the Elephant Dreams video project
#[test]
#[ignore = "this test requires the matroska-test-files submodule"]
fn ietf_test_2() {
    let mut file = std::fs::File::open("matroska-test-files/test_files/test2.mkv").unwrap();
    let ebml_head = Ebml::read_from(&mut file).unwrap();
    assert!(ebml_head.crc32.is_some(), "EBML header should have CRC-32");
    let segment = Segment::read_from(&mut file).unwrap();
    assert!(
        segment.info.crc32.is_some(),
        "Segment Info should have CRC-32"
    );
    let tags = segment.tags.first().unwrap();
    let tag = tags.tag.first().unwrap();
    let target_tag = tag.targets.target_type_value;
    assert_eq!(*target_tag, 50);
    let title = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "TITLE")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(title, Some(Some("Elephant Dream - test 2")));
    let date_released = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "DATE_RELEASED")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(date_released, Some(Some("2010")));
    let comment = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "COMMENT")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(
        comment,
        Some(Some(
            "Matroska Validation File 2, 100,000 timecode scale, odd aspect ratio, and CRC-32. Codecs are AVC and AAC"
        ))
    );
    assert!(*segment.info.timestamp_scale == 100_000);
}

// This file is using BlockGroup+Block only for audio and video frames.
// It also removes 2 bytes off each video and audio frame since they are all equal.
// These 2 bytes have to be put back in the frame before decoding. his file also contains CRC-32 values in the EBML header, the MetaSeek, the Segment Info, the Tracks and the Tags and PrevSize/Position in the Clusters for better error recovery.
// It contains H264 (1024x576 pixels), and stereo MP3. The source material is taken from the Elephant Dreams video project
#[test]
#[ignore = "this test requires the matroska-test-files submodule"]
fn ietf_test_3() {
    let mut file = std::fs::File::open("matroska-test-files/test_files/test3.mkv").unwrap();
    let ebml_head = Ebml::read_from(&mut file).unwrap();
    assert!(ebml_head.crc32.is_some(), "EBML header should have CRC-32");
    let segment = Segment::read_from(&mut file).unwrap();
    let tags = segment.tags.first().unwrap();
    let tag = tags.tag.first().unwrap();
    let target_tag = tag.targets.target_type_value;
    assert_eq!(*target_tag, 50);
    let title = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "TITLE")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(title, Some(Some("Elephant Dream - test 3")));
    let date_released = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "DATE_RELEASED")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(date_released, Some(Some("2010")));
    let comment = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "COMMENT")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(
        comment,
        Some(Some(
            "Matroska Validation File 3, header stripping on the video track and no SimpleBlock"
        ))
    );

    // It contains H264 (1024x576 pixels), and stereo MP3.
    let tracks = segment.tracks.as_ref().unwrap();
    let video_track = tracks.track_entry.iter().find(|t| *t.track_type == 1);
    assert!(video_track.is_some());
    let video_track = video_track.unwrap();
    assert_eq!(&*video_track.codec_id, "V_MPEG4/ISO/AVC");
    assert_eq!(
        video_track.video.as_ref().map(|v| *v.pixel_width),
        Some(1024)
    );
    assert_eq!(
        video_track.video.as_ref().map(|v| *v.pixel_height),
        Some(576)
    );
    let audio_track = tracks.track_entry.iter().find(|t| *t.track_type == 2);
    assert!(audio_track.is_some());
    let audio_track = audio_track.unwrap();
    assert_eq!(&*audio_track.codec_id, "A_MPEG/L3");
    assert_eq!(audio_track.audio.as_ref().map(|a| *a.channels), Some(2));
}

// This file is using the EBML feature that allows Master elements to have no known size.
// It is used for live streams because they don't know ahead of time the size of the Segment (virtually infinite) and even sometimes the size of the Clusters (no caching on the server side).
// The first timecode of the file also doesn't start at 0 since it's supposed to be a capture from something continuous.
// The SegmentInfo also doesn't contain any Duration as it is not know.
// The sample comes from the Big Buck Bunny open project. It contains Theora video (1280x720), Vorbis audio, uses only SimpleBlock (matroska DocType v2)
// A similar file can be created with mkclean using the "--live" option
#[test]
#[ignore = "this test requires the matroska-test-files submodule"]
fn ietf_test_4() {
    let mut file = std::fs::File::open("matroska-test-files/test_files/test4.mkv").unwrap();
    let _ebml_head = Ebml::read_from(&mut file).unwrap();
    let segment_header = Header::read_from(&mut file).unwrap();
    assert!(
        segment_header.size.is_unknown,
        "Segment should have unknown size"
    );

    // this segment starts with 134 bytes of junk data (0x0a)
    // since we only test the parsing ability, we can skip them
    // in real world usage, you may want to handle them properly
    std::io::copy(&mut (&mut file).take(134), &mut sink()).unwrap();

    let mut seekhead: Vec<SeekHead> = Vec::new();
    let mut info: Option<Info> = None;
    let mut clusters: Vec<Cluster> = Vec::new();
    let mut tracks: Option<Tracks> = None;
    let mut cues: Option<Cues> = None;
    let mut attachments: Option<Attachments> = None;
    let mut chapters: Option<Chapters> = None;
    let mut tags: Vec<Tags> = Vec::new();

    let file_len = file.metadata().unwrap().len();
    while file.stream_position().unwrap() < file_len {
        let elem_header = Header::read_from(&mut file).unwrap();
        match elem_header.id {
            SeekHead::ID => {
                seekhead.push(SeekHead::read_element(&elem_header, &mut file).unwrap());
            }
            Info::ID => {
                info = Some(Info::read_element(&elem_header, &mut file).unwrap());
            }
            Tracks::ID => {
                tracks = Some(Tracks::read_element(&elem_header, &mut file).unwrap());
            }
            Cues::ID => {
                cues = Some(Cues::read_element(&elem_header, &mut file).unwrap());
            }
            Attachments::ID => {
                attachments = Some(Attachments::read_element(&elem_header, &mut file).unwrap());
            }
            Chapters::ID => {
                chapters = Some(Chapters::read_element(&elem_header, &mut file).unwrap());
            }
            Tags::ID => {
                tags.push(Tags::read_element(&elem_header, &mut file).unwrap());
            }
            Cluster::ID => {
                assert!(elem_header.size.is_unknown);
                let mut cluster = Cluster::default();
                while file.stream_position().unwrap() < file_len {
                    let header = Header::read_from(&mut file).unwrap();
                    match header.id {
                        Cluster::ID => {
                            clusters.push(cluster);
                            // next cluster
                            cluster = Cluster::default()
                        }
                        Timestamp::ID => {
                            cluster.timestamp =
                                Timestamp::read_element(&header, &mut file).unwrap();
                        }

                        Position::ID => {
                            cluster.position =
                                Some(Position::read_element(&header, &mut file).unwrap());
                        }
                        PrevSize::ID => {
                            cluster.prev_size =
                                Some(PrevSize::read_element(&header, &mut file).unwrap());
                        }
                        SimpleBlock::ID => {
                            cluster.blocks.push(
                                SimpleBlock::read_element(&header, &mut file)
                                    .unwrap()
                                    .into(),
                            );
                        }
                        BlockGroup::ID => {
                            cluster
                                .blocks
                                .push(BlockGroup::read_element(&header, &mut file).unwrap().into());
                        }
                        _ => {
                            // unexpected element skip
                            std::io::copy(&mut (&mut file).take(*header.size), &mut sink())
                                .unwrap();
                        }
                    }
                }
                clusters.push(cluster);
            }
            _ => {
                panic!("Unexpected element in segment: {}", elem_header.id);
            }
        }
    }

    let segment = Segment {
        crc32: None,
        void: None,
        seek_head: seekhead,
        info: info.unwrap(),
        cluster: clusters,
        tracks,
        cues,
        attachments,
        chapters,
        tags,
    };
    // note: the file does not contain any tags
    assert_eq!(segment.tags.len(), 0);
    assert!(*segment.info.timestamp_scale == 1_000_000);
    assert!(segment.info.duration.is_none());
}

// This has a main audio track in english and a secondary audio track in english.
// It also has subtitles in English, French, German, Hungarian, Spanish, Italian and Japanese.
// The player should provide the possibility to switch between these streams.
// The sample contains H264 (1024x576 pixels), and stereo AAC and commentary in AAC+ (using SBR).
// The source material is taken from the Elephant Dreams video project
#[test]
#[ignore = "this test requires the matroska-test-files submodule"]
fn ietf_test_5() {
    let mut file = std::fs::File::open("matroska-test-files/test_files/test5.mkv").unwrap();
    let _ebml_head = Ebml::read_from(&mut file).unwrap();
    let segment = Segment::read_from(&mut file).unwrap();
    let tags = segment.tags.first().unwrap();
    let tag = tags.tag.first().unwrap();
    let target_tag = tag.targets.target_type_value;
    assert_eq!(*target_tag, 50);
    let title = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "TITLE")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(title, Some(Some("Big Buck Bunny - test 8")));
    let date_released = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "DATE_RELEASED")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(date_released, Some(Some("2010")));
    let comment = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "COMMENT")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(
        comment,
        Some(Some(
            "Matroska Validation File 8, secondary audio commentary track, misc subtitle tracks"
        ))
    );

    // It contains H264 (1024x576 pixels), and stereo AAC and commentary in AAC+ (using SBR).
    let tracks = segment.tracks.as_ref().unwrap();
    let video_track = tracks.track_entry.iter().find(|t| *t.track_type == 1);
    assert!(video_track.is_some());
    let video_track = video_track.unwrap();
    assert_eq!(&*video_track.codec_id, "V_MPEG4/ISO/AVC");
    assert_eq!(
        video_track.video.as_ref().map(|v| *v.pixel_width),
        Some(1024)
    );
    assert_eq!(
        video_track.video.as_ref().map(|v| *v.pixel_height),
        Some(576)
    );
    let audio_tracks: Vec<_> = tracks
        .track_entry
        .iter()
        .filter(|t| *t.track_type == 2)
        .collect();
    assert_eq!(audio_tracks.len(), 2);
    let audio_track = audio_tracks[0];
    assert_eq!(&*audio_track.codec_id, "A_AAC");
    assert_eq!(audio_track.audio.as_ref().map(|a| *a.channels), Some(2));
    let audio_track = audio_tracks[1];
    assert_eq!(&*audio_track.codec_id, "A_AAC");
    assert_eq!(audio_track.audio.as_ref().map(|a| *a.channels), Some(1));
    let subtitle_tracks: Vec<_> = tracks
        .track_entry
        .iter()
        .filter(|t| *t.track_type == 17)
        .collect();
    assert_eq!(subtitle_tracks.len(), 8);
    for subtitle_track in subtitle_tracks {
        assert_eq!(&*subtitle_track.codec_id, "S_TEXT/UTF8");
        assert!(subtitle_track.audio.is_none());
        assert!(subtitle_track.video.is_none());
    }
}

// test6-tag.xml
// This file is a test of the EBML parser of the player.
// The size of the Segment and Block/SimpleBlock is coded using 1 (or the minimum possible the size) and 8 bytes randomly.
// The file also have no Cues entry. So seeking should be disabled or look for Cluster boundaries in the stream (much slower than using Cues).
#[test]
#[ignore = "this test requires the matroska-test-files submodule"]
fn ietf_test_6() {
    let mut file = std::fs::File::open("matroska-test-files/test_files/test6.mkv").unwrap();
    let _ebml_head = Ebml::read_from(&mut file).unwrap();
    let segment = Segment::read_from(&mut file).unwrap();
    let tags = segment.tags.first().unwrap();
    let tag = tags.tag.first().unwrap();
    let target_tag = tag.targets.target_type_value;
    assert_eq!(*target_tag, 50);
    let title = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "TITLE")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(title, Some(Some("Big Buck Bunny - test 6")));
    let date_released = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "DATE_RELEASED")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(date_released, Some(Some("2010")));
    let comment = tag
        .simple_tag
        .iter()
        .find(|s| &*s.tag_name == "COMMENT")
        .map(|s| s.tag_string.as_deref());
    assert_eq!(
        comment,
        Some(Some(
            "Matroska Validation File 6, random length to code the size of Clusters and Blocks, no Cues for seeking"
        ))
    );
    assert!(segment.cues.is_none(), "There should be no Cues element");
}

// Note:
// This file contains junk elements (elements not defined in the specs) either at the beginning or the end of Clusters.
// These elements should be skipped. There is also an invalid element at 451417 that should be skipped until the next valid Cluster is found.
#[test]
#[ignore = "this test requires the matroska-test-files submodule"]
fn ietf_test_7() {
    let mut file = std::fs::File::open("matroska-test-files/test_files/test7.mkv").unwrap();
    let _ebml_head = Ebml::read_from(&mut file).unwrap();
    let segment = Segment::read_from(&mut file);
    assert!(
        segment.is_err(),
        "The segment should fail to parse, but not panic, as it contains junk data in clusters. The library should not be smart. Should be handle by the library user if desired, as error recovery can be very intricate(also not effective)."
    );
}

// This file has a few audio frames missing between timecodes 6.019s and 6.360s.
// The playback should not stop, and if possible the video should not be skipped where the audio is missing
// The sample contains H264 (1024x576 pixels), and stereo AAC. The source material is taken from the Elephant Dreams video project
#[test]
#[ignore = "this test requires the matroska-test-files submodule"]
fn ietf_test_8() {
    // The sample contains H264 (1024x576 pixels), and stereo AAC. The source material is taken from the Elephant Dreams video project
    let mut file = std::fs::File::open("matroska-test-files/test_files/test8.mkv").unwrap();
    let _ebml_head = Ebml::read_from(&mut file).unwrap();
    let segment = Segment::read_from(&mut file).unwrap();
    let tracks = segment.tracks.as_ref().unwrap();
    let video_track = tracks.track_entry.iter().find(|t| *t.track_type == 1);
    assert!(video_track.is_some());
    let video_track = video_track.unwrap();
    assert_eq!(&*video_track.codec_id, "V_MPEG4/ISO/AVC");
    assert_eq!(
        video_track.video.as_ref().map(|v| *v.pixel_width),
        Some(1024)
    );
    assert_eq!(
        video_track.video.as_ref().map(|v| *v.pixel_height),
        Some(576)
    );
    let audio_tracks: Vec<_> = tracks
        .track_entry
        .iter()
        .filter(|t| *t.track_type == 2)
        .collect();
    assert_eq!(audio_tracks.len(), 1);
    let audio_track = audio_tracks[0];
    assert_eq!(&*audio_track.codec_id, "A_AAC");
    assert_eq!(audio_track.audio.as_ref().map(|a| *a.channels), Some(2));
}
