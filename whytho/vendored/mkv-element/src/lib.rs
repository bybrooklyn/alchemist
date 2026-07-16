#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

mod base; // base types for Matroska elements. ie. `VInt64`, `Header`, etc.
mod element; // Element body definitions and traits.
mod error;
mod frame;

mod lacer;
mod leaf; // Leaf elements in Matroska.
mod master; // Master elements in Matroska.
mod supplement; // Supplementary elements in Matroska. Void elements, CRC-32, etc.

use bytes::*;
use coding::*;
mod coding;

// following modules are public
pub mod io;

#[cfg(feature = "utils")]
#[cfg_attr(docsrs, doc(cfg(feature = "utils")))]
pub mod view;

// Re-export common types
pub use crate::frame::*;
pub use crate::lacer::*;
pub use error::*;

/// A prelude for common types and traits.
pub mod prelude {
    pub use crate::base::*;
    pub use crate::element::*;
    pub use crate::leaf::*;
    pub use crate::master::*;
    pub use crate::supplement::*;
}
