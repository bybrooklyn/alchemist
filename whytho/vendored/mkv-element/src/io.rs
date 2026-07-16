//! I/O utilities.

/// blocking I/O implementations, supporting reading and writing.
pub mod blocking_impl {
    use crate::{
        base::Header,
        element::Element,
        master::{Cluster, Segment},
        *,
    };
    use std::io::{Read, Write};

    /// Read from a reader.
    pub trait ReadFrom: Sized {
        /// Read Self from a reader.
        fn read_from<R: Read + ?Sized>(r: &mut R) -> crate::Result<Self>;
    }

    /// Read an element from a reader provided the header.
    pub trait ReadElement: Sized + Element {
        /// Read an element from a reader provided the header.
        fn read_element<R: Read + ?Sized>(header: &Header, r: &mut R) -> crate::Result<Self> {
            let body = header.read_body(r)?;
            Self::decode_body(&mut &body[..])
        }
    }
    impl<T: Element> ReadElement for T {}

    impl Header {
        /// Read the body of the element from a reader into memory.
        pub(crate) fn read_body<R: Read + ?Sized>(&self, r: &mut R) -> crate::Result<Vec<u8>> {
            // Segment and Cluster can have unknown size, but we don't support that here.
            let size = if self.size.is_unknown && [Segment::ID, Cluster::ID].contains(&self.id) {
                return Err(crate::Error::ElementBodySizeUnknown(self.id));
            } else {
                *self.size
            };
            // we allocate 4096 bytes upfront and grow as needed
            let cap = size.min(4096) as usize;
            let mut buf = Vec::with_capacity(cap);
            let n = std::io::copy(&mut r.take(size), &mut buf)?;
            if size != n {
                return Err(crate::Error::try_get_error(size as usize, n as usize));
            }
            Ok(buf)
        }
    }

    /// Write to a writer.
    pub trait WriteTo {
        /// Write to a writer.
        fn write_to<W: Write + ?Sized>(&self, w: &mut W) -> crate::Result<()>;
    }

    impl<T: Encode> WriteTo for T {
        fn write_to<W: Write + ?Sized>(&self, w: &mut W) -> crate::Result<()> {
            //TODO should avoid the extra allocation here
            let mut buf = vec![];
            self.encode(&mut buf)?;
            w.write_all(&buf)?;
            Ok(())
        }
    }

    /// Write an element to a writer provided the header.
    pub trait WriteElement: Sized + Element {
        /// Write an element to a writer.
        fn write_element<W: Write + ?Sized>(
            &self,
            header: &Header,
            w: &mut W,
        ) -> crate::Result<()> {
            header.write_to(w)?;
            let mut buf = vec![];
            self.encode_body(&mut buf)?;
            w.write_all(&buf)?;
            Ok(())
        }
    }
    impl<T: Element> WriteElement for T {}
}
/// tokio non-blocking I/O implementations, supporting async reading and writing.
#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub mod tokio_impl {
    use crate::{
        base::Header,
        element::Element,
        master::{Cluster, Segment},
        *,
    };

    use std::future::Future;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

    /// Read from a reader asynchronously.
    pub trait AsyncReadFrom: Sized {
        /// Read Self from a reader.
        fn async_read_from<R: tokio::io::AsyncRead + Unpin + ?Sized>(
            r: &mut R,
        ) -> impl Future<Output = crate::Result<Self>>;
    }

    /// Read an element from a reader provided the header asynchronously.
    pub trait AsyncReadElement: Sized + Element {
        /// Read an element from a reader provided the header.
        fn async_read_element<R: tokio::io::AsyncRead + Unpin + ?Sized>(
            header: &Header,
            r: &mut R,
        ) -> impl std::future::Future<Output = crate::Result<Self>> {
            async {
                let body = header.read_body_tokio(r).await?;
                Self::decode_body(&mut &body[..])
            }
        }
    }
    impl<T: Element> AsyncReadElement for T {}

    /// Write to a writer asynchronously.
    pub trait AsyncWriteTo {
        /// Write to a writer asynchronously.
        fn async_write_to<W: tokio::io::AsyncWrite + Unpin + ?Sized>(
            &self,
            w: &mut W,
        ) -> impl std::future::Future<Output = crate::Result<()>>;
    }

    impl<T: Encode> AsyncWriteTo for T {
        async fn async_write_to<W: tokio::io::AsyncWrite + Unpin + ?Sized>(
            &self,
            w: &mut W,
        ) -> crate::Result<()> {
            //TODO should avoid the extra allocation here
            let mut buf = vec![];
            self.encode(&mut buf)?;
            Ok(w.write_all(&buf).await?)
        }
    }

    /// Write an element to a writer provided the header asynchronously.
    pub trait AsyncWriteElement: Sized + Element {
        /// Write an element to a writer asynchronously.
        fn async_write_element<W: tokio::io::AsyncWrite + Unpin + ?Sized>(
            &self,
            header: &Header,
            w: &mut W,
        ) -> impl std::future::Future<Output = crate::Result<()>> {
            async {
                header.async_write_to(w).await?;
                let mut buf = vec![];
                self.encode_body(&mut buf)?;
                Ok(w.write_all(&buf).await?)
            }
        }
    }
    impl<T: Element> AsyncWriteElement for T {}

    impl Header {
        /// Read the body of the element from a reader into memory.
        pub(crate) async fn read_body_tokio<R: AsyncRead + Unpin + ?Sized>(
            &self,
            r: &mut R,
        ) -> crate::Result<Vec<u8>> {
            // Segment and Cluster can have unknown size, but we don't support that here.
            let size = if self.size.is_unknown && [Segment::ID, Cluster::ID].contains(&self.id) {
                return Err(crate::Error::ElementBodySizeUnknown(self.id));
            } else {
                *self.size
            };
            // we allocate 4096 bytes upfront and grow as needed
            let cap = size.min(4096) as usize;
            let mut buf = Vec::with_capacity(cap);
            let n = tokio::io::copy(&mut r.take(size), &mut buf).await?;
            if size != n {
                return Err(crate::Error::try_get_error(size as usize, n as usize));
            }
            Ok(buf)
        }
    }
}
