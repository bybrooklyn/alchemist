use std::ops::Deref;

use crate::base::VInt64;
use crate::element::Element;

use crate::*;

/// Ebml Void element, used for padding.
///
/// ### Note:
/// Every Master element contains an optional Void element at the end of its body, which is used for padding.
/// This library automatically aggregates multiple Void elements into one at the end.
/// * When reading, all Void elements at the same level will be counted as one, sizes are accumulated.
/// * When writing, only one Void element will be written at the end, with size equal to the sum of all Void elements at the same level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Void {
    /// Size of the void element in bytes.
    pub size: u64,
}
impl Element for Void {
    const ID: VInt64 = VInt64::from_encoded(0xEC);
    fn decode_body(buf: &mut dyn Buf) -> crate::Result<Self> {
        let len = buf.remaining();
        buf.advance(len);
        Ok(Self { size: len as u64 })
    }
    fn encode_body<B: BufMut>(&self, buf: &mut B) -> crate::Result<()> {
        buf.put_slice(&vec![0; self.size as usize]);
        Ok(())
    }
}

/// CRC-32 element, used for integrity checking. The CRC-32 is stored as a little-endian u32.
///
/// ### Note:
/// * This element can be included in any Master element to provide a CRC-32 checksum of the element's data.
/// * It has to be the **first** element in the Master element's body if it is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crc32(pub u32);
impl Deref for Crc32 {
    type Target = u32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Element for Crc32 {
    const ID: VInt64 = VInt64::from_encoded(0xBF);
    fn decode_body(buf: &mut dyn Buf) -> crate::Result<Self> {
        let buf = <[u8; 4]>::decode(buf)?;
        Ok(Self(u32::from_le_bytes(buf)))
    }
    fn encode_body<B: BufMut>(&self, buf: &mut B) -> crate::Result<()> {
        buf.put_slice(&self.0.to_le_bytes());
        Ok(())
    }
}
