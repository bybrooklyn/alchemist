use crate::error::Error;
use crate::io::blocking_impl::*;

use crate::*;

use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;

/// A variable-length integer RFC 8794
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VInt64 {
    /// The decoded integer value.
    pub value: u64,
    /// Whether this VInt64 represents an unknown size.
    pub is_unknown: bool,
}

impl Display for VInt64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // write!(f, "{}", self.as_encoded())
        let encoded = self.as_encoded();
        if encoded <= 0xFF {
            write!(f, "0x{:02X}", encoded)
        } else if encoded <= 0xFFFF {
            write!(f, "0x{:04X}", encoded)
        } else if encoded <= 0xFFFFFF {
            write!(f, "0x{:06X}", encoded)
        } else if encoded <= 0xFFFFFFFF {
            write!(f, "0x{:08X}", encoded)
        } else if encoded <= 0xFFFFFFFFFF {
            write!(f, "0x{:010X}", encoded)
        } else if encoded <= 0xFFFFFFFFFFFF {
            write!(f, "0x{:012X}", encoded)
        } else if encoded <= 0xFFFFFFFFFFFFFF {
            write!(f, "0x{:014X}", encoded)
        } else {
            write!(f, "0x{:016X}", encoded)
        }
    }
}
impl Debug for VInt64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_struct("VInt64");
        if !self.is_unknown {
            t.field("value", &self.value);
        } else {
            t.field("value", &"Unknown");
        }
        t.field("memory", &format!("{}", self));
        t.finish()
    }
}

impl Deref for VInt64 {
    type Target = u64;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl VInt64 {
    /// Create a VInt64 from an already encoded u64 value.
    pub const fn from_encoded(enc: u64) -> Self {
        if enc == 0xFF {
            Self {
                value: 127,
                is_unknown: true,
            }
        } else if enc == 0x407F {
            Self {
                value: 127,
                is_unknown: false,
            }
        } else {
            Self {
                value: enc & (u64::MAX >> (enc.leading_zeros() + 1)),
                is_unknown: false,
            }
        }
    }

    /// Create a VInt64 representing an unknown size.
    pub const fn new_unknown() -> Self {
        Self {
            value: 127,
            is_unknown: true,
        }
    }

    /// Create a VInt64 from a u64 value.
    pub const fn new(value: u64) -> Self {
        Self {
            value,
            is_unknown: false,
        }
    }

    /// Create a VInt64 from an already encoded u64 value.
    pub fn as_encoded(&self) -> u64 {
        if self.is_unknown {
            return 0xFF;
        }
        if self.value == 127 {
            return 0x407F;
        }

        let size = VInt64::encode_size(self.value);
        let mut sbuf = [0u8; 8];
        let slice = &mut sbuf[8 - size..];
        slice.copy_from_slice(&self.value.to_be_bytes()[8 - size..]);
        slice[0] |= 1u8 << (8 - size);
        u64::from_be_bytes(sbuf)
    }

    /// Get the size in bytes of the encoded representation of a u64 value.
    pub const fn encode_size(value: u64) -> usize {
        let leading_zeros = value.leading_zeros() as usize;
        let total_bits = 64 - leading_zeros;
        if total_bits == 0 {
            1
        } else {
            (total_bits + 6).div_euclid(7)
        }
    }
}

impl ReadFrom for VInt64 {
    fn read_from<R: std::io::Read + ?Sized>(r: &mut R) -> crate::Result<Self> {
        let mut first_byte_buf = [0u8; 1];
        r.read_exact(&mut first_byte_buf)?;
        let first_byte = first_byte_buf[0];
        if first_byte == 0xFF {
            return Ok(VInt64 {
                value: 127,
                is_unknown: true,
            });
        }

        let leading_zeros = first_byte.leading_zeros() as usize;
        if leading_zeros >= 8 {
            return Err(crate::error::Error::InvalidVInt);
        }

        if leading_zeros == 0 {
            Ok(VInt64 {
                value: (first_byte & 0b0111_1111) as u64,
                is_unknown: false,
            })
        } else {
            let mut buf = [0u8; 8];
            let read_buf = &mut buf[8 - leading_zeros..];
            r.read_exact(read_buf)?;
            if leading_zeros != 7 {
                buf[8 - leading_zeros - 1] = first_byte & (0xFF >> (leading_zeros + 1));
            }
            Ok(VInt64 {
                value: u64::from_be_bytes(buf),
                is_unknown: false,
            })
        }
    }
}

#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
impl crate::io::tokio_impl::AsyncReadFrom for VInt64 {
    async fn async_read_from<R: tokio::io::AsyncRead + Unpin + ?Sized>(
        r: &mut R,
    ) -> crate::Result<Self> {
        let mut first_byte_buf = [0u8; 1];
        tokio::io::AsyncReadExt::read_exact(r, &mut first_byte_buf).await?;
        let first_byte = first_byte_buf[0];
        if first_byte == 0xFF {
            return Ok(VInt64 {
                value: 127,
                is_unknown: true,
            });
        }

        let leading_zeros = first_byte.leading_zeros() as usize;
        if leading_zeros >= 8 {
            return Err(crate::error::Error::InvalidVInt);
        }

        if leading_zeros == 0 {
            Ok(VInt64 {
                value: (first_byte & 0b0111_1111) as u64,
                is_unknown: false,
            })
        } else {
            let mut buf = [0u8; 8];
            let read_buf = &mut buf[8 - leading_zeros..];
            tokio::io::AsyncReadExt::read_exact(r, read_buf).await?;
            if leading_zeros != 7 {
                buf[8 - leading_zeros - 1] = first_byte & (0xFF >> (leading_zeros + 1));
            }
            Ok(VInt64 {
                value: u64::from_be_bytes(buf),
                is_unknown: false,
            })
        }
    }
}

impl Decode for VInt64 {
    fn decode(buf: &mut dyn Buf) -> crate::Result<Self> {
        let first_byte = buf.try_get_u8()?;
        if first_byte == 0 {
            return Err(Error::InvalidVInt);
        }
        if first_byte == 0xFF {
            return Ok(VInt64 {
                value: 127,
                is_unknown: true,
            });
        }
        let leading_zeros = first_byte.leading_zeros() as usize;

        if leading_zeros == 0 {
            Ok(VInt64 {
                value: (first_byte & 0b0111_1111) as u64,
                is_unknown: false,
            })
        } else {
            if buf.remaining() < leading_zeros {
                return Err(Error::try_get_error(leading_zeros, buf.remaining()));
            }
            let mut bytes = [0u8; 8];
            let read_buf = &mut bytes[8 - leading_zeros..];
            buf.take(leading_zeros).copy_to_slice(read_buf);
            if leading_zeros != 7 {
                bytes[8 - leading_zeros - 1] = first_byte & (0xFF >> (leading_zeros + 1));
            }
            Ok(VInt64 {
                value: u64::from_be_bytes(bytes),
                is_unknown: false,
            })
        }
    }
}

impl Encode for VInt64 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> crate::Result<()> {
        if self.is_unknown {
            buf.put_slice(&[0xFF]);
            return Ok(());
        }
        if self.value == 127 {
            buf.put_slice(&[0x40, 0x7F]);
            return Ok(());
        }

        let size = VInt64::encode_size(self.value);
        let mut sbuf = [0u8; 8];
        let slice = &mut sbuf[8 - size..];
        slice.copy_from_slice(&self.value.to_be_bytes()[8 - size..]);
        slice[0] |= 1u8 << (8 - size);
        buf.put_slice(slice);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn test_encode_size() {
        let test_pair = [
            (vec![0b1000_0000], 0),
            (vec![0b1000_0001], 1),
            (vec![0b0100_0000, 0xFF], 0xFF),
            (vec![0b0100_0001, 0xFF], 0b1_1111_1111),
            (vec![0b0111_1111, 0xFF], 0b11_1111_1111_1111),
            (vec![0b0010_0000, 0b0111_1111, 0xFF], 0b111_1111_1111_1111),
            (vec![0b0010_0000, 0xFF, 0xFF], 0xFFFF),
            (vec![0b0011_1111, 0xFF, 0xFF], 0b1_1111_1111_1111_1111_1111),
            (
                vec![1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                0xFF_FFFF_FFFF_FFFF,
            ),
        ];
        for (encoded, val) in test_pair {
            assert_eq!(VInt64::encode_size(val), encoded.len());
        }
    }

    #[test]
    fn test_encode() {
        let test_pair = [
            (vec![0b1000_0000], 0),
            (vec![0b1000_0001], 1),
            (vec![0b0100_0000, 0xFF], 0xFF),
            (vec![0b0100_0001, 0xFF], 0b1_1111_1111),
            (vec![0b0111_1111, 0xFF], 0b11_1111_1111_1111),
            (vec![0b0010_0000, 0b0111_1111, 0xFF], 0b111_1111_1111_1111),
            (vec![0b0010_0000, 0xFF, 0xFF], 0xFFFF),
            (vec![0b0011_1111, 0xFF, 0xFF], 0b1_1111_1111_1111_1111_1111),
            (
                vec![1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                0xFF_FFFF_FFFF_FFFF,
            ),
        ];
        for (encoded, val) in test_pair {
            let v = VInt64 {
                value: val,
                is_unknown: false,
            };
            let mut out = vec![];
            v.encode(&mut out).unwrap();
            assert_eq!(encoded, out);

            let encoded_num = v.as_encoded();
            let mut enc8 = vec![0u8; 8 - encoded.len()];
            enc8.extend_from_slice(&encoded);
            let encoded_from = u64::from_be_bytes(enc8.try_into().unwrap());
            assert_eq!(encoded_num, encoded_from);
        }
    }

    #[test]
    fn test_decode() {
        let test_pair = [
            (vec![0b1000_0000], 0),
            (vec![0b1000_0001], 1),
            (vec![0b0100_0000, 0xFF], 0xFF),
            (vec![0b0100_0001, 0xFF], 0b1_1111_1111),
            (vec![0b0111_1111, 0xFF], 0b11_1111_1111_1111),
            (vec![0b0010_0000, 0b0111_1111, 0xFF], 0b111_1111_1111_1111),
            (vec![0b0010_0000, 0xFF, 0xFF], 0xFFFF),
            (vec![0b0011_1111, 0xFF, 0xFF], 0b1_1111_1111_1111_1111_1111),
            (
                vec![1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                0xFF_FFFF_FFFF_FFFF,
            ),
        ];
        for (encoded, val) in test_pair {
            // test read
            let mut c = std::io::Cursor::new(encoded.clone());
            let vint = VInt64::read_from(&mut c).unwrap();
            assert_eq!(*vint, val);

            // test decode
            let ecoded2 = encoded.clone();
            let mut slice_encoded2 = Bytes::from(ecoded2);
            let vint_decoded = VInt64::decode(&mut slice_encoded2).unwrap();
            assert_eq!(*vint_decoded, val);

            // test from_encoded
            let mut enc8 = vec![0u8; 8 - encoded.len()];
            enc8.extend_from_slice(&encoded);
            let v = VInt64::from_encoded(u64::from_be_bytes(enc8.try_into().unwrap()));
            assert_eq!(*v, val);
        }
    }

    #[test]
    fn test_unknown() {
        let v1 = VInt64::read_from(&mut std::io::Cursor::new(vec![0xFF])).unwrap();
        let vv1 = VInt64::from_encoded(0xFF);
        assert!(v1.is_unknown);
        assert!(vv1.is_unknown);

        let v2 = VInt64::read_from(&mut std::io::Cursor::new(vec![0x80])).unwrap();
        let vv2 = VInt64::from_encoded(0x80);

        assert!(!v2.is_unknown);
        assert!(!vv2.is_unknown);

        let v3 = VInt64::read_from(&mut std::io::Cursor::new(vec![0x40, 0x7F])).unwrap();
        let vv3 = VInt64::from_encoded(0x407F);
        assert_eq!(*v3, 127);
        assert_eq!(*vv3, 127);

        assert_ne!(VInt64::new(127), VInt64::new_unknown());
        assert_eq!(VInt64::new(127).as_encoded(), 0x407F);
    }
}

/// EBML element header, consisting of an ID and a size.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Header {
    /// EBML ID of the element.
    pub id: VInt64,
    /// Size of the element's data, excluding the header itself.
    pub size: VInt64,
}

impl ReadFrom for Header {
    fn read_from<R: std::io::Read + ?Sized>(reader: &mut R) -> crate::Result<Self> {
        let id = VInt64::read_from(reader)?;
        let size = VInt64::read_from(reader)?;
        Ok(Self { id, size })
    }
}

#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
impl crate::io::tokio_impl::AsyncReadFrom for Header {
    async fn async_read_from<R: tokio::io::AsyncRead + Unpin + ?Sized>(
        r: &mut R,
    ) -> crate::Result<Self> {
        let id = VInt64::async_read_from(r).await?;
        let size = VInt64::async_read_from(r).await?;
        Ok(Self { id, size })
    }
}

impl Decode for Header {
    fn decode(buf: &mut dyn Buf) -> crate::Result<Self> {
        let id = VInt64::decode(buf)?;
        let size = VInt64::decode(buf)?;
        Ok(Self { id, size })
    }
}

impl Encode for Header {
    fn encode<B: BufMut>(&self, buf: &mut B) -> crate::Result<()> {
        self.id.encode(buf)?;
        self.size.encode(buf)?;
        Ok(())
    }
}
