//! AV2 entropy coding: the `od_ec` range encoder plus the symbol/CDF bit writer.
//!
//! Direct port of `avm/avm_dsp/{entenc.c,entcode.c,prob.h,bitwriter.c}`. This is the
//! bit-exact substrate the whole bitstream depends on, so it is implemented for real and
//! locked down with round-trip tests against a faithful decoder port before anything
//! relies on it.
#![forbid(unsafe_code)]

pub mod cdf;
pub mod entenc;
pub mod writer;

pub use cdf::{cdf_size, update_cdf};
pub use entenc::{CDF_PROB_TOP, Checkpoint, OdEcEnc, od_icdf};
pub use writer::Writer;
