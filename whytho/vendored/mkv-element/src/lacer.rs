//! Handler for lacing and delacing operations on frame data.

use crate::{Error, base::VInt64, io::blocking_impl::ReadFrom, *};

// https://www.matroska.org/technical/notes.html
/// Handler for lacing and delacing operations on frame data.
pub enum Lacer {
    /// Xiph lacing (variable-size frames with size prefixes)
    ///
    /// The Xiph lacing uses the same coding of size as found in the Ogg container \[@?RFC3533\]. The bits 5-6 of the Block Header flags are set to 01.
    /// The Block data with laced frames is stored as follows:
    ///     Lacing Head on 1 Octet: Number of frames in the lace minus 1.
    ///     Lacing size of each frame except the last one.
    ///     Binary data of each frame consecutively.
    /// The lacing size is split into 255 values, stored as unsigned octets – for example, 500 is coded 255;245 or [0xFF 0xF5]. A frame with a size multiple of 255 is coded with a 0 at the end of the size – for example, 765 is coded 255;255;255;0 or [0xFF 0xFF 0xFF 0x00].
    /// The size of the last frame is deduced from the size remaining in the Block after the other frames.
    Xiph,

    /// Fixed-size lacing (all frames have the same size)
    FixedSize,
    /// EBML lacing (variable-size frames with EBML-encoded sizes)
    ///
    /// The EBML lacing encodes the frame size with an EBML-like encoding \[@!RFC8794\]. The bits 5-6 of the Block Header flags are set to 11.
    ///
    /// The Block data with laced frames is stored as follows:
    ///     Lacing Head on 1 Octet: Number of frames in the lace minus 1.
    ///     Lacing size of each frame except the last one.
    ///     Binary data of each frame consecutively.
    ///
    /// The first frame size is encoded as an EBML Variable-Size Integer value, also known as VINT in \[@!RFC8794\].
    /// The remaining frame sizes are encoded as signed values using the difference between the frame size and the previous frame size.
    /// These signed values are encoded as VINT, with a mapping from signed to unsigned numbers.
    /// Decoding the unsigned number stored in the VINT to a signed number is done by subtracting 2^((7*n)-1)-1, where n is the octet size of the VINT.
    Ebml,
}

impl Lacer {
    /// Encode multiple frames into a single laced block
    pub fn lace(&self, frames: &[&[u8]]) -> Vec<u8> {
        if frames.is_empty() {
            return vec![];
        }
        let num_frames = frames.len();
        let mut output = vec![];
        output.push((num_frames - 1) as u8); // Number of frames - 1

        match self {
            Lacer::Xiph => {
                for frame in &frames[..num_frames - 1] {
                    let mut size = frame.len();
                    while size >= 0xFF {
                        output.push(0xFF);
                        size -= 0xFF;
                    }
                    output.push(size as u8);
                }
                for frame in frames {
                    output.extend_from_slice(frame);
                }
                output
            }
            Lacer::FixedSize => {
                let frame_size = frames[0].len();
                if let Some((idx, bad_frame)) = frames
                    .iter()
                    .enumerate()
                    .find(|(_, f)| f.len() != frame_size)
                {
                    panic!(
                        "All frames must have the same size for FixedSize lacing: expected size {}, but frame at index {} has size {}",
                        frame_size,
                        idx,
                        bad_frame.len()
                    );
                }
                for frame in frames {
                    output.extend_from_slice(frame);
                }
                output
            }
            Lacer::Ebml => {
                if num_frames == 1 {
                    output.extend_from_slice(frames[0]);
                    return output;
                }
                let sizes = frames.iter().map(|f| f.len() as u64).collect::<Vec<_>>();
                // except first size, other sizes are stored as diffs to the previous size
                let diff_sizes = std::iter::once(
                    // first
                    VInt64::new(sizes[0]),
                )
                .chain(sizes.windows(2).map(|w| {
                    let diff = w[1] as i64 - w[0] as i64;

                    //-(2^6^-1) to 2^6^
                    let n = if diff > -(2i64.pow(6) - 1) && diff < (2i64.pow(6)) {
                        1
                    } else if diff > -(2i64.pow(13) - 1) && diff < (2i64.pow(13)) {
                        2
                    } else if diff > -(2i64.pow(20) - 1) && diff < (2i64.pow(20)) {
                        3
                    } else if diff > -(2i64.pow(27) - 1) && diff < (2i64.pow(27)) {
                        4
                    } else if diff > -(2i64.pow(34) - 1) && diff < (2i64.pow(34)) {
                        5
                    } else if diff > -(2i64.pow(41) - 1) && diff < (2i64.pow(41)) {
                        6
                    } else if diff > -(2i64.pow(48) - 1) && diff < (2i64.pow(48)) {
                        7
                    } else {
                        panic!("Frame size diff too large for EBML lacing: diff = {}", diff);
                    };

                    // map to unsigned
                    let diff_unsigned = diff + (2i64.pow(7 * n as u32 - 1) - 1);
                    VInt64::new(diff_unsigned as u64)
                }))
                // dont include last size, it is deduced from remaining data
                .take(num_frames - 1);

                for size in diff_sizes {
                    size.encode(&mut output).unwrap();
                }
                for frame in frames {
                    output.extend_from_slice(frame);
                }
                output
            }
        }
    }

    /// Decode a laced block into individual frames
    pub fn delace<'a>(&self, data: &'a [u8]) -> crate::Result<Vec<&'a [u8]>> {
        // TODO(perf): avoid heap allocations ideally
        // we should be able to return a `impl Iterator<Item = crate::Result<&'a [u8]>>` here
        // can make it work using nightly features like `generators`.
        // but not sure how to do that with the current stable Rust.

        if data.is_empty() {
            return Ok(vec![]);
        }
        let num_frames = data[0] as usize + 1;
        if num_frames == 1 {
            return Ok(vec![&data[1..]]);
        }

        match self {
            Lacer::Xiph => {
                let mut out = Vec::with_capacity(num_frames);

                let data_start_pos = data
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter(|(_, b)| **b != 0xFF)
                    .nth(num_frames - 2)
                    .map(|(i, _)| i)
                    .ok_or(Error::MalformedLacingData)?
                    + 1;

                let laced_data = data
                    .get(data_start_pos..)
                    .ok_or(Error::MalformedLacingData)?;

                let mut start = 0;
                for size in data[1..data_start_pos]
                    .split_inclusive(|b| *b != 0xFF)
                    .map(|chunk| chunk.iter().map(|b| *b as usize).sum::<usize>())
                {
                    out.push(
                        laced_data
                            .get(start..start + size)
                            .ok_or(Error::MalformedLacingData)?,
                    );
                    start += size;
                }
                out.push(laced_data.get(start..).ok_or(Error::MalformedLacingData)?);
                Ok(out)
            }
            Lacer::FixedSize => {
                let data_len = data.len() - 1;

                // all frames must have the same size
                if !data_len.is_multiple_of(num_frames) {
                    return Err(Error::MalformedLacingData);
                }

                Ok(data[1..].chunks(data_len / num_frames).collect())
            }
            Lacer::Ebml => {
                let mut data_buf = &data[1..];
                let mut out_sizes = Vec::with_capacity(num_frames - 1);
                let first_size = VInt64::read_from(&mut data_buf)?;
                out_sizes.push(*first_size as usize);
                for _ in 1..(num_frames - 1) {
                    let oct_size = data_buf
                        .first()
                        .ok_or(Error::MalformedLacingData)?
                        .leading_zeros()
                        + 1;
                    let current_encoded_vint = VInt64::read_from(&mut data_buf)?;
                    // unsigned to signed
                    let diff = *current_encoded_vint as i64 - (2i64.pow(7 * oct_size - 1) - 1);
                    let new_size = out_sizes
                        .last()
                        .unwrap()
                        .checked_add_signed(diff as isize)
                        .ok_or(Error::MalformedLacingData)?;
                    out_sizes.push(new_size);
                }

                let mut out = Vec::with_capacity(num_frames);

                let mut start = 0;
                for size in out_sizes {
                    out.push(
                        data_buf
                            .get(start..start + size)
                            .ok_or(Error::MalformedLacingData)?,
                    );
                    start += size;
                }
                out.push(data_buf.get(start..).ok_or(Error::MalformedLacingData)?);
                Ok(out)
            }
        }
    }
}

#[cfg(test)]
mod lacer_tests {
    use super::*;
    #[test]
    fn test_xiph_lacing() {
        // 0 frames
        let laced = Lacer::Xiph.lace(&[]);
        assert_eq!(laced, vec![]);
        let frames: Vec<_> = Lacer::Xiph.delace(&[]).unwrap();
        assert_eq!(frames.len(), 0);

        // 4 frames, sizes: 255, 256, 1, remaining
        let len = vec![0x03, 0xFF, 0x00, 0xFF, 0x1, 0x1];
        let frame0 = vec![2u8; 255];
        let frame1 = vec![42u8; 256];
        let frame2 = vec![38u8; 1];
        let frame3 = vec![100u8; 1];

        let laced = Lacer::Xiph.lace(&[&frame0, &frame1, &frame2, &frame3]);
        let data = [len, frame0, frame1, frame2, frame3].concat();
        assert_eq!(laced, data);

        let frames: Vec<_> = Lacer::Xiph.delace(&data).unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0], &[2u8; 255]);
        assert_eq!(frames[1], &[42u8; 256]);
        assert_eq!(frames[2], &[38u8; 1]);
        assert_eq!(frames[3], &[100u8; 1]);

        // 1 frame, size: remaining
        let len = vec![0x00];
        let frame0 = vec![2u8; 255];

        let laced = Lacer::Xiph.lace(&[&frame0]);
        let data = [len, frame0].concat();
        assert_eq!(laced, data);

        let frames: Vec<_> = Lacer::Xiph.delace(&data).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], &[2u8; 255]);

        // 2 frames, sizes: 32, remaining
        let len = vec![0x01, 0x20];
        let frame0 = vec![2u8; 32];
        let frame1 = vec![42u8; 256];

        let laced = Lacer::Xiph.lace(&[&frame0, &frame1]);
        let data = [len, frame0, frame1].concat();
        assert_eq!(laced, data);

        let frames: Vec<_> = Lacer::Xiph.delace(&data).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], &[2u8; 32]);
        assert_eq!(frames[1], &[42u8; 256]);

        // 4 frames, sizes: 600, 3, 520, remaining
        let len = vec![0x03, 0xFF, 0xFF, 0x5A, 0x3, 0xFF, 0xFF, 0xA];
        assert_eq!(0xff + 0xff + 0x5A, 600);
        assert_eq!(0xff + 0xff + 0xA, 520);
        let frame0 = vec![2u8; 600];
        let frame1 = vec![42u8; 3];
        let frame2 = vec![38u8; 520];
        let frame3 = vec![100u8; 1];

        let laced = Lacer::Xiph.lace(&[&frame0, &frame1, &frame2, &frame3]);
        let data = [len, frame0, frame1, frame2, frame3].concat();
        assert_eq!(laced, data);

        let frames: Vec<_> = Lacer::Xiph.delace(&data).unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0], &[2u8; 600]);
        assert_eq!(frames[1], &[42u8; 3]);
        assert_eq!(frames[2], &[38u8; 520]);
        assert_eq!(frames[3], &[100u8; 1]);
    }

    #[test]
    fn test_ebml_lacing() {
        // 0 frames
        let laced = Lacer::Ebml.lace(&[]);
        assert_eq!(laced, vec![]);
        let frames: Vec<_> = Lacer::Ebml.delace(&[]).unwrap();
        assert_eq!(frames.len(), 0);

        // 3 frames, sizes: 800, 500, remaining(1000)

        // store as size diffs: 800, -300

        // offset = 2**(7*n - 1) - 1
        // n = 2 -> 2**13 - 1 = 8191
        // convert to uint: 800, 7891(-300+8191)

        // encode as VInt:
        // 0x4320(800), 0x5ED3(7891)

        let len = vec![0x02, 0x43, 0x20, 0x5E, 0xD3];
        let frame0 = vec![2u8; 800];
        let frame1 = vec![42u8; 500];
        let frame2 = vec![38u8; 1000];
        let laced = Lacer::Ebml.lace(&[&frame0, &frame1, &frame2]);
        let data = [len, frame0, frame1, frame2].concat();
        assert_eq!(laced, data);

        let frames: Vec<_> = Lacer::Ebml.delace(&data).unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], &[2u8; 800]);
        assert_eq!(frames[1], &[42u8; 500]);
        assert_eq!(frames[2], &[38u8; 1000]);

        // 7 frames, sizes      2, 5000, 4980, 400, 20, 2000, remaining(300)
        // store as size diffs: 2, 4998, -20, -4580, -380, 1980
        let len = vec![
            0x06, 0x82, 0x73, 0x85, 0xAB, 0x4E, 0x1B, 0x5E, 0x83, 0x67, 0xBB,
        ];
        let frame0 = vec![2u8; 2];
        let frame1 = vec![42u8; 5000];
        let frame2 = vec![38u8; 4980];
        let frame3 = vec![100u8; 400];
        let frame4 = vec![7u8; 20];
        let frame5 = vec![8u8; 2000];
        let frame6 = vec![9u8; 300];
        let laced = Lacer::Ebml.lace(&[
            &frame0, &frame1, &frame2, &frame3, &frame4, &frame5, &frame6,
        ]);
        let data = [len, frame0, frame1, frame2, frame3, frame4, frame5, frame6].concat();
        assert_eq!(laced, data);
        let frames: Vec<_> = Lacer::Ebml.delace(&data).unwrap();
        assert_eq!(frames.len(), 7);
        assert_eq!(frames[0], &[2u8; 2]);
        assert_eq!(frames[1], &[42u8; 5000]);
        assert_eq!(frames[2], &[38u8; 4980]);
        assert_eq!(frames[3], &[100u8; 400]);
        assert_eq!(frames[4], &[7u8; 20]);
        assert_eq!(frames[5], &[8u8; 2000]);
        assert_eq!(frames[6], &[9u8; 300]);
    }

    #[test]
    fn test_fixed_size_lacing() {
        // 0 frames
        let laced = Lacer::FixedSize.lace(&[]);
        assert_eq!(laced, vec![]);
        let frames: Vec<_> = Lacer::FixedSize.delace(&[]).unwrap();
        assert_eq!(frames.len(), 0);

        // 3 frames, sizes: 500, 500, 500
        let len = vec![0x02];
        let frame0 = vec![2u8; 500];
        let frame1 = vec![42u8; 500];
        let frame2 = vec![38u8; 500];
        let laced = Lacer::FixedSize.lace(&[&frame0, &frame1, &frame2]);
        let data = [len, frame0, frame1, frame2].concat();
        assert_eq!(laced, data);

        let frames: Vec<_> = Lacer::FixedSize.delace(&data).unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], &[2u8; 500]);
        assert_eq!(frames[1], &[42u8; 500]);
        assert_eq!(frames[2], &[38u8; 500]);
    }
}
