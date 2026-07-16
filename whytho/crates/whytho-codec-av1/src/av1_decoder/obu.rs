//! OBU (Open Bitstream Unit) parser for AV1.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OBUType {
    Reserved0,
    SequenceHeader,
    TemporalDelimiter,
    FrameHeader,
    TileGroup,
    Metadata,
    Frame,
    Padding,
    Reserved(u8),
}

impl OBUType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Reserved0,
            1 => Self::SequenceHeader,
            2 => Self::TemporalDelimiter,
            3 => Self::FrameHeader,
            4 => Self::TileGroup,
            5 => Self::Metadata,
            6 => Self::Frame,
            15 => Self::Padding,
            _ => Self::Reserved(value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OBU {
    pub obu_type: OBUType,
    pub has_size: bool,
    pub temporal_id: u8,
    pub spatial_id: u8,
    pub payload: Vec<u8>,
}

pub fn parse_obus(data: &[u8]) -> Result<Vec<OBU>, String> {
    let mut obus = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        if pos + 1 >= data.len() {
            break;
        }

        let header_byte = data[pos];
        pos += 1;

        let obu_type = OBUType::from_u8((header_byte >> 3) & 0x0F);
        let has_size = (header_byte >> 1) & 1 == 1;
        let temporal_id = 0;
        let spatial_id = 0;

        if !has_size {
            // OBU without size — not standard for most types
            return Err("OBU without size field not supported".into());
        }

        // Read LEB128 size
        let (size, bytes_read) = read_leb128(&data[pos..])?;
        pos += bytes_read;

        if pos + size as usize > data.len() {
            return Err(format!(
                "OBU payload extends past data: need {size} bytes at pos {pos}, have {}",
                data.len() - pos
            ));
        }

        let payload = data[pos..pos + size as usize].to_vec();
        pos += size as usize;

        obus.push(OBU {
            obu_type,
            has_size,
            temporal_id,
            spatial_id,
            payload,
        });
    }

    Ok(obus)
}

fn read_leb128(data: &[u8]) -> Result<(u64, usize), String> {
    let mut value = 0u64;
    let mut bytes_read = 0;

    for &byte in data.iter().take(8) {
        value |= ((byte & 0x7F) as u64) << (bytes_read * 7);
        bytes_read += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }

    Ok((value, bytes_read))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leb128_single_byte() {
        let data = [0x42];
        let (value, bytes_read) = read_leb128(&data).unwrap();
        assert_eq!(value, 0x42);
        assert_eq!(bytes_read, 1);
    }

    #[test]
    fn leb128_multi_byte() {
        let data = [0x81, 0x01];
        let (value, bytes_read) = read_leb128(&data).unwrap();
        assert_eq!(value, 0x81);
        assert_eq!(bytes_read, 2);
    }

    #[test]
    fn obu_type_from_u8() {
        assert_eq!(OBUType::from_u8(1), OBUType::SequenceHeader);
        assert_eq!(OBUType::from_u8(6), OBUType::Frame);
        assert_eq!(OBUType::from_u8(2), OBUType::TemporalDelimiter);
    }
}
