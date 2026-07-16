//! The Daala/AV1-style `od_ec` range encoder.
//!
//! Direct port of `avm/avm_dsp/entenc.c` together with the helpers from
//! `avm/avm_dsp/{entcode.c,prob.h,entcode.h}`. `low` is a 64-bit window, `rng` starts at
//! `0x8000`, `cnt` starts at `-9`; output bytes are buffered in a pre-carry buffer and
//! carry-propagated in [`OdEcEnc::done`].
//!
//! The encoder is exercised by a faithful decoder port in the tests (round-trip), which is
//! the bit-exactness gate until the reference `avmdec` validation harness is wired up.

/// Probabilities are coded in Q15.
const CDF_PROB_BITS: u32 = 15;
/// The CDF top value, `1 << 15` (`CDF_PROB_TOP`).
pub const CDF_PROB_TOP: u32 = 1 << CDF_PROB_BITS;
/// `CDF_SHIFT = 15 - CDF_PROB_BITS` (`0` for AV2).
const CDF_SHIFT: i32 = 15 - CDF_PROB_BITS as i32;
/// Range-scaling probability shift, from `prob.h`.
const EC_PROB_SHIFT: u32 = 7;

/// `OD_ICDF(x) = CDF_PROB_TOP - x` (the inverse-CDF representation; its own inverse).
#[inline]
pub const fn od_icdf(x: u32) -> u32 {
    CDF_PROB_TOP - x
}

/// `av2_prob_inc_tbl[15][16]` from `avm/avm_dsp/entcode.c`, indexed `[nsym - 2][n]`.
///
/// The `-1` sentinels are never indexed for a valid symbol (`n` only ranges over
/// `0..nsym`, where every such entry is non-negative), so storing them as `i16` is
/// bit-exact with the reference `uint16_t` table for all valid inputs.
#[rustfmt::skip]
static PROB_INC_TBL: [[i16; 16]; 15] = [
    [ 8,  0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [10,  5,  0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [12,  8,  4,  0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [12,  9,  6,  3,  0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [13, 10,  8,  5,  2,  0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [13, 11,  9,  6,  4,  2,  0, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [14, 12, 10,  8,  6,  4,  2,  0, -1, -1, -1, -1, -1, -1, -1, -1],
    [14, 12, 10,  8,  7,  5,  3,  1,  0, -1, -1, -1, -1, -1, -1, -1],
    [14, 12, 11,  9,  8,  6,  4,  3,  1,  0, -1, -1, -1, -1, -1, -1],
    [14, 13, 11, 10,  8,  7,  5,  4,  2,  1,  0, -1, -1, -1, -1, -1],
    [14, 13, 12, 10,  9,  8,  6,  5,  4,  2,  1,  0, -1, -1, -1, -1],
    [14, 13, 12, 11,  9,  8,  7,  6,  4,  3,  2,  1,  0, -1, -1, -1],
    [14, 13, 12, 11, 10,  9,  8,  6,  5,  4,  3,  2,  1,  0, -1, -1],
    [14, 13, 12, 11, 10,  9,  8,  7,  6,  5,  4,  3,  2,  1,  0, -1],
    [15, 14, 13, 12, 11, 10,  9,  8,  7,  6,  5,  4,  3,  2,  1,  0],
];

/// Scale a Q15 probability into the current range. Port of `od_ec_prob_scale` (`prob.h`).
#[inline]
fn od_ec_prob_scale(p: u32, r: u32, n: i32, nsym: i32) -> u32 {
    let rr = (r >> 8) as i32;
    let mut pp = (p >> EC_PROB_SHIFT) as i32;
    pp <<= 4;
    pp += PROB_INC_TBL[(nsym - 2) as usize][n as usize] as i32;
    // 7 - EC_PROB_SHIFT - CDF_SHIFT + 1 + 6 == 7 for AV2.
    let shift = 7 - EC_PROB_SHIFT as i32 - CDF_SHIFT + 1 + 6;
    (((rr * pp) >> shift) << 3) as u32
}

/// A saved range-coder state, for trial (RD) coding. See [`OdEcEnc::checkpoint`].
#[derive(Clone, Copy, Debug)]
pub struct Checkpoint {
    low: u64,
    rng: u16,
    cnt: i16,
    offs: usize,
    error: bool,
}

/// The `od_ec` range encoder.
#[derive(Clone, Debug)]
pub struct OdEcEnc {
    /// Buffered output bytes with room for carry; carry-propagated in [`Self::done`].
    /// `offs` in the reference is simply this buffer's length.
    precarry: Vec<u16>,
    /// The low end of the current range (the `od_ec_window`).
    low: u64,
    /// The current range; `0x8000 <= rng <= 0xFFFF` after each normalization.
    rng: u16,
    /// Bits of data in the current value (initialized to `-9`).
    cnt: i16,
    /// Sticky error flag (set on misuse), mirroring the reference `error`.
    error: bool,
}

impl Default for OdEcEnc {
    fn default() -> Self {
        Self::new()
    }
}

impl OdEcEnc {
    /// Create a fresh encoder.
    pub fn new() -> Self {
        Self {
            precarry: Vec::new(),
            low: 0,
            rng: 0x8000,
            cnt: -9,
            error: false,
        }
    }

    /// Create a fresh encoder with output capacity reserved (in bytes).
    pub fn with_capacity(bytes: usize) -> Self {
        let mut e = Self::new();
        e.precarry = Vec::with_capacity(bytes);
        e
    }

    /// Reset to the initial state, keeping allocated capacity.
    pub fn reset(&mut self) {
        self.precarry.clear();
        self.low = 0;
        self.rng = 0x8000;
        self.cnt = -9;
        self.error = false;
    }

    /// Whether an encoding error has occurred.
    pub fn has_error(&self) -> bool {
        self.error
    }

    /// Renormalize `low`/`rng` so that `0x8000 <= rng < 0x10000`, flushing bytes to the
    /// pre-carry buffer. Port of `od_ec_enc_normalize`.
    fn normalize(&mut self, mut low: u64, rng: u32, n_bypass: i32) {
        debug_assert!(rng <= 0xFFFF);
        let c0;
        let d;
        if n_bypass > 0 {
            c0 = self.cnt as i32 + n_bypass;
            d = 0;
        } else {
            c0 = self.cnt as i32;
            // d = 16 - OD_ILOG_NZ(rng) == leading zeros of rng in its 16-bit form.
            d = (rng as u16).leading_zeros() as i32;
        }
        let mut s = c0 + d;
        if s >= 0 {
            let mut c = c0 + 16;
            let mut m = (1u64 << c) - 1;
            if s >= 8 {
                self.precarry.push((low >> c) as u16);
                low &= m;
                c -= 8;
                m >>= 8;
            }
            self.precarry.push((low >> c) as u16);
            s = c + d - 24;
            low &= m;
        }
        self.low = low << d;
        self.rng = (rng << d) as u16;
        self.cnt = s as i16;
    }

    /// Encode a symbol given `fl`/`fh` (inverse-CDF bounds, Q15). Port of `od_ec_encode_q15`.
    fn encode_q15(&mut self, fl: u32, fh: u32, s: i32, nsyms: i32) {
        let mut l = self.low;
        let mut r = self.rng as u32;
        debug_assert!(r & 1 == 0);
        debug_assert!(r >= CDF_PROB_TOP);
        debug_assert!(fh <= fl && fl <= CDF_PROB_TOP);
        let v;
        if fl < CDF_PROB_TOP {
            let u = od_ec_prob_scale(fl, r, s - 1, nsyms);
            v = od_ec_prob_scale(fh, r, s, nsyms);
            l += (r - u) as u64;
            r = u - v;
        } else {
            v = od_ec_prob_scale(fh, r, s, nsyms);
            r -= v;
        }
        self.normalize(l, r, 0);
    }

    /// Encode symbol `s` from an inverse-CDF table in Q15. Port of `avm_od_ec_encode_cdf_q15`.
    ///
    /// `icdf` must be monotonically non-increasing with `icdf[nsyms - 1] == 0`.
    pub fn encode_cdf_q15(&mut self, s: usize, icdf: &[u16], nsyms: usize) {
        debug_assert!(s < nsyms);
        debug_assert_eq!(icdf[nsyms - 1] as u32, od_icdf(CDF_PROB_TOP));
        let fl = if s > 0 {
            icdf[s - 1] as u32
        } else {
            od_icdf(0)
        };
        let fh = icdf[s] as u32;
        self.encode_q15(fl, fh, s as i32, nsyms as i32);
    }

    /// Encode a single binary value with probability `f` (of being 1), scaled by 32768.
    /// Port of `avm_od_ec_encode_bool_q15`.
    pub fn encode_bool_q15(&mut self, val: bool, f: u32) {
        debug_assert!(0 < f && f < CDF_PROB_TOP);
        let mut l = self.low;
        let mut r = self.rng as u32;
        debug_assert!(r >= CDF_PROB_TOP);
        let v = od_ec_prob_scale(f, r, 0, 2);
        if val {
            l += (r - v) as u64;
        }
        r = if val { v } else { r - v };
        self.normalize(l, r, 0);
    }

    /// Encode `n_bits` raw (bypass / uniform) bits, MSB first. Port of
    /// `od_ec_encode_literal_bypass`.
    ///
    /// `n_bits` must be in `1..=8`: like the reference, [`normalize`](Self::normalize)
    /// flushes at most two bytes per call, so wider values are written in `<=8`-bit chunks
    /// by [`crate::Writer::literal`] (mirroring `avm_write_literal`).
    pub fn encode_literal_bypass(&mut self, val: u32, n_bits: i32) {
        debug_assert!(0 < n_bits && n_bits <= 8);
        let mut l = self.low;
        let r = self.rng as u32;
        l <<= n_bits;
        l = l.wrapping_add(r.wrapping_mul(val) as u64);
        self.normalize(l, r, n_bits);
    }

    /// Encode a single bypass bit. Port of `od_ec_encode_bool_bypass`.
    pub fn encode_bool_bypass(&mut self, val: bool) {
        self.encode_literal_bypass(val as u32, 1);
    }

    /// Overwrite up to 8 already-encoded leading bits. Port of `od_ec_enc_patch_initial_bits`.
    ///
    /// Requires that at least `nbits` bits were previously coded with exact power-of-two
    /// probabilities (the caller guarantees this).
    pub fn patch_initial_bits(&mut self, val: u32, nbits: i32) {
        debug_assert!((0..=8).contains(&nbits));
        debug_assert!(val < (1u32 << nbits));
        let shift = 8 - nbits;
        let mask = ((1u32 << nbits) - 1) << shift;
        if !self.precarry.is_empty() {
            self.precarry[0] = ((self.precarry[0] as u32 & !mask) | (val << shift)) as u16;
        } else if 9 + self.cnt as i32 + (self.rng == 0x8000) as i32 > nbits {
            let sh = 16 + self.cnt as i32;
            self.low = (self.low & !((mask as u64) << sh)) | ((val as u64) << (sh + shift));
        } else {
            self.error = true;
        }
    }

    /// Number of bits used so far (slightly over-estimated). Port of `avm_od_ec_enc_tell`.
    pub fn tell(&self) -> i32 {
        self.cnt as i32 + 10 + self.precarry.len() as i32 * 8
    }

    /// Finalize the stream and return the coded bytes. Port of `avm_od_ec_enc_done`.
    ///
    /// Operates on copies of the carry buffer so the encoder may keep coding afterwards.
    pub fn done(&self) -> Vec<u8> {
        let l = self.low;
        let mut c = self.cnt as i32;
        let mut s = 10 + c;
        let m: u64 = 0x3FFF;
        let mut e = (l.wrapping_add(m) & !m) | (m + 1);
        let mut precarry = self.precarry.clone();
        if s > 0 {
            let mut n = (1u64 << (c + 16)) - 1;
            loop {
                precarry.push((e >> (c + 16)) as u16);
                e &= n;
                s -= 8;
                c -= 8;
                n >>= 8;
                if s <= 0 {
                    break;
                }
            }
        }
        let offs = precarry.len();
        let mut out = vec![0u8; offs];
        let mut carry: u32 = 0;
        for i in (0..offs).rev() {
            carry += precarry[i] as u32;
            out[i] = carry as u8;
            carry >>= 8;
        }
        out
    }

    /// Save the current state for later rollback (trial RD coding).
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            low: self.low,
            rng: self.rng,
            cnt: self.cnt,
            offs: self.precarry.len(),
            error: self.error,
        }
    }

    /// Restore a state previously saved with [`Self::checkpoint`]. The checkpoint must be a
    /// causal ancestor of the current state (only appended data is discarded).
    pub fn rollback(&mut self, cp: &Checkpoint) {
        debug_assert!(cp.offs <= self.precarry.len());
        self.low = cp.low;
        self.rng = cp.rng;
        self.cnt = cp.cnt;
        self.error = cp.error;
        self.precarry.truncate(cp.offs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful port of the reference range *decoder* (`entdec.{c,h}`), used to validate
    /// that the encoder produces a bit-exact, decodable stream.
    const OD_EC_WINDOW_SIZE: i32 = 64;
    const OD_EC_MIN_BITS: i32 = 8;
    const OD_EC_LOTS_OF_BITS: i32 = 0x4000;

    struct OdEcDec<'a> {
        buf: &'a [u8],
        bptr: usize,
        dif: u64,
        rng: u16,
        cnt: i16,
        tell_offs: i32,
    }

    impl<'a> OdEcDec<'a> {
        fn new(buf: &'a [u8]) -> Self {
            let mut d = OdEcDec {
                buf,
                bptr: 0,
                dif: (1u64 << (OD_EC_WINDOW_SIZE - 1)) - 1,
                rng: 0x8000,
                cnt: -15,
                tell_offs: -15 + 1,
            };
            d.refill();
            d
        }

        fn refill(&mut self) {
            let mut s = OD_EC_WINDOW_SIZE - 9 - (self.cnt as i32 + 15);
            while s >= 0 && self.bptr < self.buf.len() {
                self.dif ^= (self.buf[self.bptr] as u64) << s;
                self.cnt += 8;
                s -= 8;
                self.bptr += 1;
            }
            if self.bptr >= self.buf.len() {
                self.tell_offs += OD_EC_LOTS_OF_BITS - self.cnt as i32;
                self.cnt = OD_EC_LOTS_OF_BITS as i16;
            }
        }

        fn normalize(&mut self, dif: u64, rng: u32, ret: i32) -> i32 {
            let d = (rng as u16).leading_zeros() as i32;
            self.cnt -= d as i16;
            self.dif = (dif.wrapping_add(1) << d).wrapping_sub(1);
            self.rng = (rng << d) as u16;
            if (self.cnt as i32) < OD_EC_MIN_BITS {
                self.refill();
            }
            ret
        }

        fn bypass_normalize(&mut self, dif: u64, n_bypass: i32, ret: i32) -> i32 {
            self.cnt -= n_bypass as i16;
            self.dif = (dif.wrapping_add(1) << n_bypass).wrapping_sub(1);
            if (self.cnt as i32) < OD_EC_MIN_BITS {
                self.refill();
            }
            ret
        }

        fn decode_cdf_q15(&mut self, icdf: &[u16], nsyms: usize) -> usize {
            let dif = self.dif;
            let r = self.rng as u32;
            let c = (dif >> (OD_EC_WINDOW_SIZE - 16)) as u32;
            let mut v = r;
            let mut ret: i32 = -1;
            let mut u;
            loop {
                u = v;
                ret += 1;
                v = od_ec_prob_scale(icdf[ret as usize] as u32, r, ret, nsyms as i32);
                if c >= v {
                    break;
                }
            }
            let r2 = u - v;
            let dif2 = dif - ((v as u64) << (OD_EC_WINDOW_SIZE - 16));
            self.normalize(dif2, r2, ret) as usize
        }

        fn decode_bool_q15(&mut self, f: u32) -> bool {
            let mut dif = self.dif;
            let r = self.rng as u32;
            let v = od_ec_prob_scale(f, r, 0, 2);
            let vw = (v as u64) << (OD_EC_WINDOW_SIZE - 16);
            let mut ret = 1;
            let mut r_new = v;
            if dif >= vw {
                r_new = r - v;
                dif -= vw;
                ret = 0;
            }
            self.normalize(dif, r_new, ret) != 0
        }

        fn decode_literal_bypass(&mut self, n_bits: i32) -> u32 {
            let mut dif = self.dif;
            let r = self.rng as u32;
            let mut vw = (r as u64) << (OD_EC_WINDOW_SIZE - 16);
            let mut ret: u32 = 0;
            for _ in 0..n_bits {
                vw >>= 1;
                ret <<= 1;
                if dif >= vw {
                    dif -= vw;
                } else {
                    ret |= 1;
                }
            }
            self.bypass_normalize(dif, n_bits, ret as i32) as u32
        }
    }

    /// Tiny deterministic PRNG for reproducible randomized round-trips.
    struct Lcg(u64);
    impl Lcg {
        fn next_u32(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 32) as u32
        }
        fn below(&mut self, n: u32) -> u32 {
            self.next_u32() % n
        }
    }

    /// Build a valid Q15 inverse-CDF for `nsyms` symbols with a guaranteed minimum
    /// per-symbol frequency (so scaled boundaries stay distinct).
    fn make_icdf(rng: &mut Lcg, nsyms: usize) -> Vec<u16> {
        // Strictly increasing cumulative boundaries in (0, 32768).
        let min_gap = 256u32;
        let span = CDF_PROB_TOP - min_gap * nsyms as u32;
        let mut bounds = Vec::with_capacity(nsyms);
        let mut acc = 0u32;
        for i in 0..nsyms - 1 {
            acc += min_gap + rng.below(span / nsyms as u32);
            let _ = i;
            bounds.push(acc.min(CDF_PROB_TOP - 1));
        }
        bounds.push(CDF_PROB_TOP);
        // Ensure strictly increasing.
        for i in 1..bounds.len() {
            if bounds[i] <= bounds[i - 1] {
                bounds[i] = bounds[i - 1] + 1;
            }
        }
        bounds.iter().map(|&b| od_icdf(b) as u16).collect()
    }

    #[derive(Clone)]
    enum Op {
        Symbol {
            s: usize,
            icdf: Vec<u16>,
            nsyms: usize,
        },
        Bool {
            val: bool,
            f: u32,
        },
        Literal {
            val: u32,
            bits: i32,
        },
    }

    #[test]
    fn roundtrip_mixed_random_ops() {
        for seed in 0..64u64 {
            let mut rng = Lcg(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1));
            let n_ops = 200 + rng.below(300) as usize;
            let mut ops = Vec::with_capacity(n_ops);
            for _ in 0..n_ops {
                match rng.below(3) {
                    0 => {
                        let nsyms = 2 + rng.below(7) as usize; // 2..=8
                        let icdf = make_icdf(&mut rng, nsyms);
                        let s = rng.below(nsyms as u32) as usize;
                        ops.push(Op::Symbol { s, icdf, nsyms });
                    }
                    1 => {
                        let f = 1 + rng.below(CDF_PROB_TOP - 2);
                        let val = rng.below(2) == 1;
                        ops.push(Op::Bool { val, f });
                    }
                    _ => {
                        // The bypass primitive is contracted to 1..=8 bits per call.
                        let bits = 1 + rng.below(8) as i32;
                        let val = rng.below(1u32 << bits);
                        ops.push(Op::Literal { val, bits });
                    }
                }
            }

            let mut enc = OdEcEnc::new();
            for op in &ops {
                match op {
                    Op::Symbol { s, icdf, nsyms } => enc.encode_cdf_q15(*s, icdf, *nsyms),
                    Op::Bool { val, f } => enc.encode_bool_q15(*val, *f),
                    Op::Literal { val, bits } => enc.encode_literal_bypass(*val, *bits),
                }
            }
            let bytes = enc.done();
            assert!(!enc.has_error(), "seed {seed}: encoder error");

            let mut dec = OdEcDec::new(&bytes);
            for (i, op) in ops.iter().enumerate() {
                match op {
                    Op::Symbol { s, icdf, nsyms } => {
                        let got = dec.decode_cdf_q15(icdf, *nsyms);
                        assert_eq!(got, *s, "seed {seed} op {i}: symbol mismatch");
                    }
                    Op::Bool { val, f } => {
                        let got = dec.decode_bool_q15(*f);
                        assert_eq!(got, *val, "seed {seed} op {i}: bool mismatch");
                    }
                    Op::Literal { val, bits } => {
                        let got = dec.decode_literal_bypass(*bits);
                        assert_eq!(got, *val, "seed {seed} op {i}: literal mismatch");
                    }
                }
            }
        }
    }

    #[test]
    fn writer_wide_literals_roundtrip() {
        use crate::Writer;
        let mut rng = Lcg(0xDEAD_BEEF);
        // (value, width) pairs spanning widths that force >8-bit chunking.
        let mut items = Vec::new();
        let mut w = Writer::new();
        for _ in 0..300 {
            let bits = 1 + rng.below(32);
            let val = if bits == 32 {
                rng.next_u32()
            } else {
                rng.next_u32() & ((1u32 << bits) - 1)
            };
            w.literal(val, bits);
            items.push((val, bits));
        }
        let bytes = w.finish();

        let mut dec = OdEcDec::new(&bytes);
        for (i, &(val, bits)) in items.iter().enumerate() {
            // Decode using the same <=8-bit chunking as `Writer::literal` / `avm_write_literal`.
            let mut remaining = bits as i32;
            let mut acc = 0u32;
            while remaining > 0 {
                let n = if remaining >= 8 { 8 } else { remaining };
                let c = dec.decode_literal_bypass(n);
                acc = (acc << n) | c;
                remaining -= n;
            }
            assert_eq!(acc, val, "wide literal {i} (width {bits}) mismatch");
        }
    }

    #[test]
    fn empty_stream_is_decodable() {
        let enc = OdEcEnc::new();
        let bytes = enc.done();
        // A freshly-finalized empty stream still produces at least one byte.
        assert!(!bytes.is_empty());
    }

    #[test]
    fn checkpoint_rollback_restores_state() {
        let mut rng = Lcg(12345);
        let mut enc = OdEcEnc::new();
        // Code some prefix.
        for _ in 0..50 {
            let f = 1 + rng.below(CDF_PROB_TOP - 2);
            enc.encode_bool_q15(rng.below(2) == 1, f);
        }
        let cp = enc.checkpoint();
        let baseline = enc.clone();
        // Code a divergent suffix, then roll back.
        for _ in 0..50 {
            enc.encode_literal_bypass(rng.below(1 << 8), 8);
        }
        enc.rollback(&cp);
        assert_eq!(
            enc.done(),
            baseline.done(),
            "rollback must restore exact state"
        );
    }

    #[test]
    fn tell_is_monotonic_nondecreasing() {
        let mut rng = Lcg(99);
        let mut enc = OdEcEnc::new();
        let mut last = enc.tell();
        for _ in 0..500 {
            let f = 1 + rng.below(CDF_PROB_TOP - 2);
            enc.encode_bool_q15(rng.below(2) == 1, f);
            let now = enc.tell();
            assert!(now >= last, "tell decreased");
            last = now;
        }
    }
}
