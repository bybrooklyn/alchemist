use crate::base::VInt64;

/// Error types for this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// I/O error, from `std::io::Error`.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid variable-length integer encoding, incidicates a vint longer than 8 bytes.
    #[error("Invalid variable-length integer encoding, 8 leading zeros found...")]
    InvalidVInt,

    /// Attempted to read past the end of the buffer.
    #[error("Attempted to read past the end of the buffer")]
    TryGetError(#[from] bytes::TryGetError),

    /// Attempted to read past the end of the buffer during element body decoding.
    #[error("Element body over decode, ID: {0}")]
    OverDecode(VInt64),

    /// Not all bytes were consumed in a element body
    #[error("Short read: not all bytes were consumed")]
    ShortRead,

    /// Not all bytes were consumed in a element body during element body decoding.
    #[error("Element body under decode, ID: {0}")]
    UnderDecode(VInt64),

    /// Missing element.
    #[error("Missing element, ID: {0}")]
    MissingElement(VInt64),

    /// Duplicate element in a master element.
    #[error("Duplicate element {id} in master element {parent}")]
    DuplicateElement {
        /// The duplicate element ID.
        id: VInt64,
        /// The parent master element ID.
        parent: VInt64,
    },

    /// Element body size is unknown.
    #[error("Element body size is unknown, ID: {0}")]
    ElementBodySizeUnknown(VInt64),

    /// Malformed lacing data.
    #[error("Malformed lacing data")]
    MalformedLacingData,
}

impl Error {
    /// Helper function to create a TryGetError with the requested and available sizes.
    #[inline]
    pub fn try_get_error(requested: usize, available: usize) -> Self {
        Error::TryGetError(bytes::TryGetError {
            requested,
            available,
        })
    }
}

/// Result type for this crate.
pub type Result<T> = std::result::Result<T, Error>;
