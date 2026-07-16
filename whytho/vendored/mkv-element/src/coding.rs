//! Encoding and decoding Element or other types from buffers in memory.
use crate::*;

/// Decode an element from a buffer.
pub trait Decode: Sized {
    /// Decode an element from the buffer.
    fn decode(buf: &mut dyn Buf) -> Result<Self>;
}

impl<const N: usize> Decode for [u8; N] {
    fn decode(buf: &mut dyn Buf) -> Result<Self> {
        if buf.remaining() < N {
            return Err(Error::try_get_error(N, buf.remaining()));
        }
        let mut v = [0u8; N];
        buf.take(N).copy_to_slice(&mut v);
        Ok(v)
    }
}

/// Encode an element to a buffer.
pub trait Encode {
    /// Encode self to the buffer.
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()>;
}

impl<T: Encode> Encode for &[T] {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        for item in self.iter() {
            item.encode(buf)?;
        }

        Ok(())
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        match self {
            Some(v) => v.encode(buf),
            None => Ok(()),
        }
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        for item in self.iter() {
            item.encode(buf)?;
        }

        Ok(())
    }
}
