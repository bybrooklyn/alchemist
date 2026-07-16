//! CDF context layout and the probability adaptation step.
//!
//! Port of `update_cdf` from `avm/avm_dsp/prob.h`. An adaptive inverse-CDF for `nsyms`
//! symbols occupies `CDF_SIZE(nsyms) = nsyms + 4` `u16` slots:
//!   * `[0 .. nsyms]`     — the inverse-CDF entries (the last one is `0`),
//!   * `[nsyms]`          — the adaptation counter,
//!   * `[nsyms+1 ..= nsyms+3]` — three per-context PARA rate offsets (time intervals 0/1/2).

use crate::entenc::od_icdf;

/// Number of `u16` slots an adaptive CDF for `nsyms` symbols occupies (`CDF_SIZE`).
pub const fn cdf_size(nsyms: usize) -> usize {
    nsyms + 4
}

/// Adapt the inverse-CDF in place after coding symbol `val`. Port of `update_cdf`.
///
/// The final inverse-CDF entry (`cdf[nsyms - 1] == 0`) is left untouched, preserving the
/// table invariant; the counter at `cdf[nsyms]` saturates at 32.
pub fn update_cdf(cdf: &mut [u16], val: usize, nsyms: usize) {
    debug_assert!((2..17).contains(&nsyms));
    let count = cdf[nsyms];
    let time_interval = if count > 31 {
        2
    } else if count > 15 {
        1
    } else {
        0
    };
    let rate = 2 + cdf[nsyms + 1 + time_interval] as i32;
    // tmp is CDF_PROB_TOP for symbols before `val`, and 0 from `val` onward.
    let mut tmp = od_icdf(0) as i32;
    for (i, slot) in cdf[..nsyms - 1].iter_mut().enumerate() {
        if i == val {
            tmp = 0;
        }
        let c = *slot as i32;
        if tmp < c {
            *slot = (c - ((c - tmp) >> rate)) as u16;
        } else {
            *slot = (c + ((tmp - c) >> rate)) as u16;
        }
    }
    if cdf[nsyms] < 32 {
        cdf[nsyms] += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entenc::CDF_PROB_TOP;

    /// Build a 3-symbol adaptive CDF (equal initial split) with PARA rate slots.
    fn cdf3() -> Vec<u16> {
        let third = CDF_PROB_TOP / 3;
        let mut v = vec![
            od_icdf(third) as u16,        // icdf[0]
            od_icdf(2 * third) as u16,    // icdf[1]
            od_icdf(CDF_PROB_TOP) as u16, // icdf[2] == 0
            0,                            // counter
            4,                            // PARA t0
            5,                            // PARA t1
            6,                            // PARA t2
        ];
        assert_eq!(v.len(), cdf_size(3));
        v[2] = 0;
        v
    }

    #[test]
    fn update_preserves_invariants() {
        let mut cdf = cdf3();
        for _ in 0..1000 {
            update_cdf(&mut cdf, 0, 3);
        }
        // Last inverse-CDF entry stays 0.
        assert_eq!(cdf[2], 0);
        // Inverse-CDF is non-increasing.
        assert!(cdf[0] >= cdf[1] && cdf[1] >= cdf[2]);
        // Counter saturates at 32.
        assert_eq!(cdf[3], 32);
    }

    #[test]
    fn favored_symbol_shifts_mass() {
        let mut cdf = cdf3();
        let before = cdf[0];
        // Symbol 0 occupies [0, CDF_PROB_TOP - icdf[0]); coding it repeatedly should grow
        // its probability, i.e. lower icdf[0].
        for _ in 0..200 {
            update_cdf(&mut cdf, 0, 3);
        }
        assert!(cdf[0] < before, "favoring symbol 0 should reduce icdf[0]");
    }
}
