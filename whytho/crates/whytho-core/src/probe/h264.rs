use std::convert::TryFrom;

use crate::error::WhyThoError;

use super::HdrInfo;

pub fn parse_codec_private(
    codec_private: &[u8],
    codec_id: &str,
) -> Result<
    (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<u8>,
        Option<HdrInfo>,
    ),
    WhyThoError,
> {
    match codec_id {
        "V_MPEG4/ISO/AVC" => parse_avcc(codec_private),
        _ => Ok((None, None, None, None, None)),
    }
}

fn parse_avcc(
    data: &[u8],
) -> Result<
    (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<u8>,
        Option<HdrInfo>,
    ),
    WhyThoError,
> {
    use h264_reader::avcc::AvcDecoderConfigurationRecord;
    use h264_reader::nal::sps::SeqParameterSet;
    use h264_reader::nal::{Nal, RefNal};

    let avcc =
        AvcDecoderConfigurationRecord::try_from(data).map_err(|e| WhyThoError::CorruptStream {
            track: 0,
            reason: format!("AVCC parse error: {e:?}"),
        })?;

    let mut profile_str = None;
    let mut level_str = None;
    let mut pixel_format = None;
    let mut bit_depth = None;
    let mut hdr = None;

    for sps_bytes in avcc.sequence_parameter_sets() {
        let sps_bytes = sps_bytes.map_err(|e| WhyThoError::CorruptStream {
            track: 0,
            reason: format!("SPS read error: {e:?}"),
        })?;

        let nal = RefNal::new(sps_bytes, &[], true);
        let sps = SeqParameterSet::from_bits(nal.rbsp_bits()).map_err(|e| {
            WhyThoError::CorruptStream {
                track: 0,
                reason: format!("SPS parse error: {e:?}"),
            }
        })?;

        let profile = sps.profile();
        profile_str = Some(format!("{profile:?}"));

        let level = sps.level();
        level_str = Some(format!(
            "{}.{}",
            level.level_idc() / 10,
            level.level_idc() % 10
        ));

        {
            let chroma_info = &sps.chroma_info;
            pixel_format = Some(format!("{:?}", chroma_info.chroma_format));
            let luma_depth = 8 + chroma_info.bit_depth_luma_minus8 as u8;
            if luma_depth > 8 {
                bit_depth = Some(luma_depth);
            }
        }

        if let Some(ref vui) = sps.vui_parameters {
            if let Some(ref vst) = vui.video_signal_type {
                if let Some(ref cd) = vst.colour_description {
                    let transfer = cd.transfer_characteristics;
                    hdr = Some(HdrInfo {
                        format: match transfer {
                            16 => Some("PQ (HDR10)".into()),
                            18 => Some("HLG".into()),
                            _ => None,
                        },
                        color_primaries: Some(format!("colour_primaries={}", cd.colour_primaries)),
                        transfer_characteristics: Some(format!("transfer={transfer}")),
                    });
                }
            }
        }

        break;
    }

    Ok((profile_str, level_str, pixel_format, bit_depth, hdr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_avcc_returns_error() {
        let result = parse_avcc(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_non_av1_codec_private_returns_none() {
        let result = parse_codec_private(&[0x01, 0x02], "V_VP9").unwrap();
        assert_eq!(result, (None, None, None, None, None));
    }
}
