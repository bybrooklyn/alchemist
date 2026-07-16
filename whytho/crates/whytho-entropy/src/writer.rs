//! Symbol writer over the range encoder.
//!
//! Port of `avm/avm_dsp/bitwriter.{h,c}`: adaptive CDF symbols, explicit-probability bits,
//! and raw/bypass literals, all on top of [`OdEcEnc`]. `allow_update_cdf` mirrors the
//! reference flag that disables adaptation (e.g. for some bypass contexts).

use crate::cdf::update_cdf;
use crate::entenc::OdEcEnc;

/// A bit writer that codes symbols/bits into an AV2 range-coded stream.
#[derive(Clone, Debug)]
pub struct Writer {
    enc: OdEcEnc,
    allow_update_cdf: bool,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    /// Create a writer with CDF adaptation enabled.
    pub fn new() -> Self {
        Self {
            enc: OdEcEnc::new(),
            allow_update_cdf: true,
        }
    }

    /// Create a writer with output capacity reserved (in bytes).
    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            enc: OdEcEnc::with_capacity(bytes),
            allow_update_cdf: true,
        }
    }

    /// Enable/disable CDF adaptation after each adaptive symbol.
    pub fn set_allow_update_cdf(&mut self, allow: bool) {
        self.allow_update_cdf = allow;
    }

    /// Write symbol `s` using an adaptive CDF (layout per [`crate::cdf::cdf_size`]) and adapt it.
    pub fn symbol(&mut self, s: usize, cdf: &mut [u16], nsyms: usize) {
        self.enc.encode_cdf_q15(s, &cdf[..nsyms], nsyms);
        if self.allow_update_cdf {
            update_cdf(cdf, s, nsyms);
        }
    }

    /// Write symbol `s` using a fixed (non-adaptive) inverse-CDF.
    pub fn symbol_fixed(&mut self, s: usize, icdf: &[u16], nsyms: usize) {
        self.enc.encode_cdf_q15(s, icdf, nsyms);
    }

    /// Write a single bit with explicit Q15 probability `f` of being 1.
    pub fn bool_prob(&mut self, val: bool, f: u32) {
        self.enc.encode_bool_q15(val, f);
    }

    /// Write `nbits` raw bits, MSB first. Wide values are split into `<=8`-bit chunks,
    /// exactly as `avm_write_literal` does (the bypass primitive accepts at most 8 bits).
    pub fn literal(&mut self, val: u32, nbits: u32) {
        let mut bits = nbits as i32;
        let mut data = val;
        while bits > 0 {
            let n = if bits >= 8 { 8 } else { bits };
            self.enc.encode_literal_bypass(data >> (bits - n), n);
            bits -= n;
            data &= (1u32 << bits).wrapping_sub(1);
        }
    }

    /// Write a single bypass (uniform 50/50) bit.
    pub fn bit(&mut self, val: bool) {
        self.enc.encode_bool_bypass(val);
    }

    /// Bits used so far (an over-estimate, per the reference `tell`).
    pub fn tell(&self) -> i32 {
        self.enc.tell()
    }

    /// Borrow the underlying range encoder (e.g. to patch leading bits).
    pub fn encoder_mut(&mut self) -> &mut OdEcEnc {
        &mut self.enc
    }

    /// Finalize the stream and return the coded bytes.
    pub fn finish(self) -> Vec<u8> {
        self.enc.done()
    }
}
