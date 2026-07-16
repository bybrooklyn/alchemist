//! CAVLC (Context-Adaptive Variable-Length Coding) for H.264 encoder.
//!
//! Implements coefficient token coding for residual blocks per H.264 spec.

use super::BitstreamWriter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    Luma4x4,
    LumaDC16x16,
    ChromaDC,
    ChromaAC,
}

/// Coefficient token VLC table for 0 <= nC < 2 (Table 9-5(a)).
#[rustfmt::skip]
static COEFF_TOKEN_NC0: &[(u32, u8, u8, u8)] = &[
    (0b1, 1, 0, 0),
    (0b000101, 6, 1, 0), (0b01, 2, 1, 1),
    (0b00000111, 8, 2, 0), (0b000100, 6, 2, 1), (0b001, 3, 2, 2),
    (0b000000111, 9, 3, 0), (0b00000110, 8, 3, 1), (0b0000101, 7, 3, 2), (0b00011, 5, 3, 3),
    (0b0000000111, 10, 4, 0), (0b000000110, 9, 4, 1), (0b00000101, 8, 4, 2), (0b000011, 6, 4, 3),
    (0b00000000111, 11, 5, 0), (0b0000000110, 10, 5, 1), (0b000000101, 9, 5, 2), (0b0000100, 7, 5, 3),
    (0b0000000001111, 13, 6, 0), (0b00000000110, 11, 6, 1), (0b0000000101, 10, 6, 2), (0b00000100, 8, 6, 3),
    (0b0000000001011, 13, 7, 0), (0b0000000001110, 13, 7, 1), (0b00000000101, 11, 7, 2), (0b000000100, 9, 7, 3),
    (0b0000000001000, 13, 8, 0), (0b0000000001010, 13, 8, 1), (0b0000000001101, 13, 8, 2), (0b0000000100, 10, 8, 3),
    (0b00000000001111, 14, 9, 0), (0b00000000001110, 14, 9, 1), (0b0000000001001, 13, 9, 2), (0b00000000100, 11, 9, 3),
    (0b00000000001011, 14, 10, 0), (0b00000000001010, 14, 10, 1), (0b00000000001101, 14, 10, 2), (0b0000000001100, 13, 10, 3),
    (0b000000000001111, 15, 11, 0), (0b000000000001110, 15, 11, 1), (0b00000000001001, 14, 11, 2), (0b00000000001100, 14, 11, 3),
    (0b000000000001011, 15, 12, 0), (0b000000000001010, 15, 12, 1), (0b000000000001101, 15, 12, 2), (0b00000000001000, 14, 12, 3),
    (0b0000000000001111, 16, 13, 0), (0b000000000000001, 15, 13, 1), (0b000000000001001, 15, 13, 2), (0b000000000001100, 15, 13, 3),
    (0b0000000000001011, 16, 14, 0), (0b0000000000001110, 16, 14, 1), (0b0000000000001101, 16, 14, 2), (0b000000000001000, 15, 14, 3),
    (0b0000000000000111, 16, 15, 0), (0b0000000000001010, 16, 15, 1), (0b0000000000001001, 16, 15, 2), (0b0000000000001100, 16, 15, 3),
    (0b0000000000000100, 16, 16, 0), (0b0000000000000110, 16, 16, 1), (0b0000000000000101, 16, 16, 2), (0b0000000000001000, 16, 16, 3),
];

/// Coefficient token VLC table for 2 <= nC < 4 (Table 9-5(b)).
#[rustfmt::skip]
static COEFF_TOKEN_NC2: &[(u32, u8, u8, u8)] = &[
    (0b11, 2, 0, 0),
    (0b001011, 6, 1, 0), (0b10, 2, 1, 1),
    (0b000111, 6, 2, 0), (0b00111, 5, 2, 1), (0b011, 3, 2, 2),
    (0b0000111, 7, 3, 0), (0b001010, 6, 3, 1), (0b001001, 6, 3, 2), (0b0101, 4, 3, 3),
    (0b00000111, 8, 4, 0), (0b000110, 6, 4, 1), (0b000101, 6, 4, 2), (0b0100, 4, 4, 3),
    (0b00000100, 8, 5, 0), (0b0000110, 7, 5, 1), (0b0000101, 7, 5, 2), (0b00110, 5, 5, 3),
    (0b000000111, 9, 6, 0), (0b00000110, 8, 6, 1), (0b00000101, 8, 6, 2), (0b001000, 6, 6, 3),
    (0b00000001111, 11, 7, 0), (0b000000110, 9, 7, 1), (0b000000101, 9, 7, 2), (0b000100, 6, 7, 3),
    (0b00000001011, 11, 8, 0), (0b00000001110, 11, 8, 1), (0b00000001101, 11, 8, 2), (0b0000100, 7, 8, 3),
    (0b000000001111, 12, 9, 0), (0b00000001010, 11, 9, 1), (0b00000001001, 11, 9, 2), (0b000000100, 9, 9, 3),
    (0b000000001011, 12, 10, 0), (0b000000001110, 12, 10, 1), (0b000000001101, 12, 10, 2), (0b00000001100, 11, 10, 3),
    (0b000000001000, 12, 11, 0), (0b000000001010, 12, 11, 1), (0b000000001001, 12, 11, 2), (0b00000001000, 11, 11, 3),
    (0b0000000001111, 13, 12, 0), (0b0000000001110, 13, 12, 1), (0b0000000001101, 13, 12, 2), (0b000000001100, 12, 12, 3),
    (0b0000000001011, 13, 13, 0), (0b0000000001010, 13, 13, 1), (0b0000000001001, 13, 13, 2), (0b0000000001100, 13, 13, 3),
    (0b0000000000111, 13, 14, 0), (0b00000000001011, 14, 14, 1), (0b0000000000110, 13, 14, 2), (0b0000000001000, 13, 14, 3),
    (0b00000000001001, 14, 15, 0), (0b00000000001000, 14, 15, 1), (0b00000000001010, 14, 15, 2), (0b0000000000001, 13, 15, 3),
    (0b00000000000111, 14, 16, 0), (0b00000000000110, 14, 16, 1), (0b00000000000101, 14, 16, 2), (0b00000000000100, 14, 16, 3),
];

/// Coefficient token VLC table for 4 <= nC < 8 (Table 9-5(c)).
#[rustfmt::skip]
static COEFF_TOKEN_NC4: &[(u32, u8, u8, u8)] = &[
    (0b1111, 4, 0, 0),
    (0b001111, 6, 1, 0), (0b1110, 4, 1, 1),
    (0b001011, 6, 2, 0), (0b01111, 5, 2, 1), (0b1101, 4, 2, 2),
    (0b001000, 6, 3, 0), (0b01100, 5, 3, 1), (0b01110, 5, 3, 2), (0b1100, 4, 3, 3),
    (0b0001111, 7, 4, 0), (0b01010, 5, 4, 1), (0b01011, 5, 4, 2), (0b1011, 4, 4, 3),
    (0b0001011, 7, 5, 0), (0b01000, 5, 5, 1), (0b01001, 5, 5, 2), (0b1010, 4, 5, 3),
    (0b0001001, 7, 6, 0), (0b001110, 6, 6, 1), (0b001101, 6, 6, 2), (0b1001, 4, 6, 3),
    (0b0001000, 7, 7, 0), (0b001010, 6, 7, 1), (0b001001, 6, 7, 2), (0b1000, 4, 7, 3),
    (0b00001111, 8, 8, 0), (0b0001110, 7, 8, 1), (0b0001101, 7, 8, 2), (0b01101, 5, 8, 3),
    (0b00001011, 8, 9, 0), (0b00001110, 8, 9, 1), (0b0001010, 7, 9, 2), (0b001100, 6, 9, 3),
    (0b000001111, 9, 10, 0), (0b00001010, 8, 10, 1), (0b00001101, 8, 10, 2), (0b0001100, 7, 10, 3),
    (0b000001011, 9, 11, 0), (0b000001110, 9, 11, 1), (0b00001001, 8, 11, 2), (0b00001100, 8, 11, 3),
    (0b000001000, 9, 12, 0), (0b000001010, 9, 12, 1), (0b000001101, 9, 12, 2), (0b00001000, 8, 12, 3),
    (0b0000001101, 10, 13, 0), (0b000000111, 9, 13, 1), (0b000001001, 9, 13, 2), (0b000001100, 9, 13, 3),
    (0b0000001001, 10, 14, 0), (0b0000001100, 10, 14, 1), (0b0000001011, 10, 14, 2), (0b0000001010, 10, 14, 3),
    (0b0000000101, 10, 15, 0), (0b0000001000, 10, 15, 1), (0b0000000111, 10, 15, 2), (0b0000000110, 10, 15, 3),
    (0b0000000001, 10, 16, 0), (0b0000000100, 10, 16, 1), (0b0000000011, 10, 16, 2), (0b0000000010, 10, 16, 3),
];

/// Chroma DC coefficient token table (Table 9-5(e)). nC >= 8 uses a 6-bit FLC handled
/// directly in `write_coeff_token`, so no VLC table is needed for it.
#[rustfmt::skip]
static COEFF_TOKEN_CHROMA_DC: &[(u32, u8, u8, u8)] = &[
    (0b01,       2, 0, 0),
    (0b000111,   6, 1, 0), (0b1,        1, 1, 1),
    (0b000100,   6, 2, 0), (0b000110,   6, 2, 1), (0b001,      3, 2, 2),
    (0b000011,   6, 3, 0), (0b0000011,  7, 3, 1), (0b0000010,  7, 3, 2), (0b000101,   6, 3, 3),
    (0b000010,   6, 4, 0), (0b00000011, 8, 4, 1), (0b00000010, 8, 4, 2), (0b0000000,  7, 4, 3),
];

/// Look up and write the coefficient token for the given nC context.
/// nC < 0 means chroma DC context.
#[allow(
    non_snake_case,
    reason = "nC mirrors the H.264 spec's own name for this value"
)]
fn write_coeff_token(w: &mut BitstreamWriter, nC: i32, total_coeff: u32, trailing_ones: u32) {
    // nC >= 8: 6-bit fixed-length code (H.264 Table 9-5, last column). The inverse of
    // the decoder's mapping: TC=0 -> 3, otherwise code = (TC-1)*4 + TrailingOnes.
    if nC >= 8 {
        let code = if total_coeff == 0 {
            3
        } else {
            (total_coeff - 1) * 4 + trailing_ones
        };
        w.write_bits(code, 6);
        return;
    }

    let table: &[(u32, u8, u8, u8)] = if nC < 0 {
        COEFF_TOKEN_CHROMA_DC
    } else if nC < 2 {
        COEFF_TOKEN_NC0
    } else if nC < 4 {
        COEFF_TOKEN_NC2
    } else {
        COEFF_TOKEN_NC4
    };

    for &(code, bits, tc, to) in table {
        if tc as u32 == total_coeff && to as u32 == trailing_ones {
            w.write_bits(code, bits);
            return;
        }
    }
    // Fallback for total_coeff=0
    if total_coeff == 0 {
        // Use the first entry (TC=0, TO=0) from the appropriate table
        let (code, bits, _, _) = table[0];
        w.write_bits(code, bits);
    }
}

/// Find the cbp_code that encodes the given cbp_luma and cbp_chroma.
/// The decoder's CBP table maps code_number -> CBP value,
/// where CBP = (cbp_chroma << 4) | cbp_luma.
/// We search for the code whose decoded CBP matches our target.
pub fn find_cbp_code(cbp_luma: u32, cbp_chroma: u32, is_intra: bool) -> u32 {
    let target_cbp = ((cbp_chroma << 4) | cbp_luma) as u8;

    // Decoder's CBP_INTRA_TABLE (H.264 Table 9-4, I-slice column)
    // Index = code_number from ue(v), value = CBP byte
    const CBP_INTRA_TABLE: [u8; 48] = [
        47, 31, 15, 0, 23, 27, 29, 30, 7, 11, 13, 14, 39, 43, 45, 46, 16, 3, 5, 10, 12, 19, 21, 26,
        28, 35, 37, 42, 44, 1, 2, 4, 8, 17, 18, 20, 24, 6, 9, 22, 25, 32, 33, 34, 36, 40, 38, 41,
    ];

    // Decoder's CBP_INTER_TABLE (H.264 Table 9-4, P/B-slice column)
    // Index = code_number from ue(v), value = CBP byte
    const CBP_INTER_TABLE: [u8; 48] = [
        0, 16, 1, 2, 4, 8, 32, 3, 5, 10, 12, 15, 47, 7, 11, 13, 14, 6, 9, 31, 35, 37, 42, 44, 33,
        34, 36, 40, 39, 43, 45, 46, 17, 18, 20, 24, 19, 21, 26, 28, 23, 27, 29, 30, 22, 25, 38, 41,
    ];

    let table = if is_intra {
        &CBP_INTRA_TABLE
    } else {
        &CBP_INTER_TABLE
    };

    for (code, &cbp) in table.iter().enumerate() {
        if cbp == target_cbp {
            return code as u32;
        }
    }
    4 // fallback: cbp_chroma=1, cbp_luma=0 (no luma, chroma DC only)
}

/// Write CAVLC coded block pattern for I16x16 macroblock.
/// Returns the CBP value used for mb_type encoding.
pub fn write_cbp(w: &mut BitstreamWriter, cbp_luma: u32, cbp_chroma: u32) {
    // CBP table for I-slices (Table 9-4 in H.264 spec)
    // cbp_luma: 0-15 (which 8x8 luma blocks have non-zero coefficients)
    // cbp_chroma: 0 (no chroma DC/AC), 1 (chroma DC only), 2 (chroma DC+AC)
    let cbp = cbp_chroma * 16 + cbp_luma;

    // Simplified CBP coding for I-slices
    match cbp {
        0 => w.write_ue(0), // No blocks
        1 => w.write_ue(1),
        2 => w.write_ue(2),
        3 => w.write_ue(3),
        4 => w.write_ue(4),
        5 => w.write_ue(5),
        6 => w.write_ue(6),
        7 => w.write_ue(7),
        8 => w.write_ue(8),
        9 => w.write_ue(9),
        10 => w.write_ue(10),
        11 => w.write_ue(11),
        12 => w.write_ue(12),
        13 => w.write_ue(13),
        14 => w.write_ue(14),
        15 => w.write_ue(15),
        16 => w.write_ue(16),
        17 => w.write_ue(17),
        18 => w.write_ue(18),
        19 => w.write_ue(19),
        20 => w.write_ue(20),
        21 => w.write_ue(21),
        22 => w.write_ue(22),
        23 => w.write_ue(23),
        24 => w.write_ue(24),
        25 => w.write_ue(25),
        26 => w.write_ue(26),
        27 => w.write_ue(27),
        28 => w.write_ue(28),
        29 => w.write_ue(29),
        30 => w.write_ue(30),
        31 => w.write_ue(31),
        32 => w.write_ue(32),
        33 => w.write_ue(33),
        34 => w.write_ue(34),
        35 => w.write_ue(35),
        36 => w.write_ue(36),
        37 => w.write_ue(37),
        38 => w.write_ue(38),
        39 => w.write_ue(39),
        40 => w.write_ue(40),
        41 => w.write_ue(41),
        42 => w.write_ue(42),
        43 => w.write_ue(43),
        44 => w.write_ue(44),
        45 => w.write_ue(45),
        46 => w.write_ue(46),
        47 => w.write_ue(47),
        _ => w.write_ue(0), // fallback
    }
}

// ============================================================
// total_zeros VLC tables (H.264 Tables 9-7, 9-8)
// Format: (value, codeword, bit_length)
// ============================================================

#[rustfmt::skip]
static TOTAL_ZEROS_1: &[(u8, u32, u8)] = &[
    (0,  0b1, 1), (1,  0b011, 3), (2,  0b010, 3), (3,  0b0011, 4),
    (4,  0b0010, 4), (5,  0b00011, 5), (6,  0b00010, 5), (7,  0b000011, 6),
    (8,  0b000010, 6), (9,  0b0000011, 7), (10, 0b0000010, 7), (11, 0b00000011, 8),
    (12, 0b00000010, 8), (13, 0b000000011, 9), (14, 0b000000010, 9), (15, 0b000000001, 9),
];

#[rustfmt::skip]
static TOTAL_ZEROS_2: &[(u8, u32, u8)] = &[
    (0,  0b111, 3), (1,  0b110, 3), (2,  0b101, 3), (3,  0b100, 3),
    (4,  0b011, 3), (5,  0b0101, 4), (6,  0b0100, 4), (7,  0b0011, 4),
    (8,  0b0010, 4), (9,  0b00011, 5), (10, 0b00010, 5), (11, 0b000011, 6),
    (12, 0b000010, 6), (13, 0b000001, 6), (14, 0b000000, 6),
];

#[rustfmt::skip]
static TOTAL_ZEROS_3: &[(u8, u32, u8)] = &[
    (0,  0b0101, 4), (1,  0b111, 3), (2,  0b110, 3), (3,  0b101, 3),
    (4,  0b0100, 4), (5,  0b0011, 4), (6,  0b100, 3), (7,  0b011, 3),
    (8,  0b0010, 4), (9,  0b00011, 5), (10, 0b00010, 5), (11, 0b000001, 6),
    (12, 0b00001, 5), (13, 0b000000, 6),
];

#[rustfmt::skip]
static TOTAL_ZEROS_4: &[(u8, u32, u8)] = &[
    (0,  0b00011, 5), (1,  0b111, 3), (2,  0b0101, 4), (3,  0b0100, 4),
    (4,  0b110, 3), (5,  0b101, 3), (6,  0b100, 3), (7,  0b0011, 4),
    (8,  0b011, 3), (9,  0b0010, 4), (10, 0b00010, 5), (11, 0b00001, 5),
    (12, 0b00000, 5),
];

#[rustfmt::skip]
static TOTAL_ZEROS_5: &[(u8, u32, u8)] = &[
    (0,  0b0101, 4), (1,  0b0100, 4), (2,  0b0011, 4), (3,  0b111, 3),
    (4,  0b110, 3), (5,  0b101, 3), (6,  0b100, 3), (7,  0b011, 3),
    (8,  0b0010, 4), (9,  0b00001, 5), (10, 0b0001, 4), (11, 0b00000, 5),
];

#[rustfmt::skip]
static TOTAL_ZEROS_6: &[(u8, u32, u8)] = &[
    (0,  0b000001, 6), (1,  0b00001, 5), (2,  0b111, 3), (3,  0b110, 3),
    (4,  0b101, 3), (5,  0b100, 3), (6,  0b011, 3), (7,  0b010, 3),
    (8,  0b0001, 4), (9,  0b001, 3), (10, 0b000000, 6),
];

#[rustfmt::skip]
static TOTAL_ZEROS_7: &[(u8, u32, u8)] = &[
    (0,  0b000001, 6), (1,  0b00001, 5), (2,  0b101, 3), (3,  0b100, 3),
    (4,  0b011, 3), (5,  0b11, 2), (6,  0b010, 3), (7,  0b0001, 4),
    (8,  0b001, 3), (9,  0b000000, 6),
];

#[rustfmt::skip]
static TOTAL_ZEROS_8: &[(u8, u32, u8)] = &[
    (0,  0b000001, 6), (1,  0b0001, 4), (2,  0b00001, 5), (3,  0b011, 3),
    (4,  0b11, 2), (5,  0b10, 2), (6,  0b010, 3), (7,  0b001, 3),
    (8,  0b000000, 6),
];

#[rustfmt::skip]
static TOTAL_ZEROS_9: &[(u8, u32, u8)] = &[
    (0,  0b000001, 6), (1,  0b000000, 6), (2,  0b0001, 4), (3,  0b11, 2),
    (4,  0b10, 2), (5,  0b001, 3), (6,  0b01, 2), (7,  0b00001, 5),
];

#[rustfmt::skip]
static TOTAL_ZEROS_10: &[(u8, u32, u8)] = &[
    (0,  0b00001, 5), (1,  0b00000, 5), (2,  0b001, 3), (3,  0b11, 2),
    (4,  0b10, 2), (5,  0b01, 2), (6,  0b0001, 4),
];

#[rustfmt::skip]
static TOTAL_ZEROS_11: &[(u8, u32, u8)] = &[
    (0,  0b0000, 4), (1,  0b0001, 4), (2,  0b001, 3), (3,  0b010, 3),
    (4,  0b1, 1), (5,  0b011, 3),
];

#[rustfmt::skip]
static TOTAL_ZEROS_12: &[(u8, u32, u8)] = &[
    (0,  0b0000, 4), (1,  0b0001, 4), (2,  0b01, 2), (3,  0b1, 1),
    (4,  0b001, 3),
];

#[rustfmt::skip]
static TOTAL_ZEROS_13: &[(u8, u32, u8)] = &[
    (0,  0b000, 3), (1,  0b001, 3), (2,  0b1, 1), (3,  0b01, 2),
];

#[rustfmt::skip]
static TOTAL_ZEROS_14: &[(u8, u32, u8)] = &[
    (0,  0b00, 2), (1,  0b01, 2), (2,  0b1, 1),
];

#[rustfmt::skip]
static TOTAL_ZEROS_15: &[(u8, u32, u8)] = &[
    (0,  0b0, 1), (1,  0b1, 1),
];

fn get_total_zeros_table(total_coeff: u32) -> &'static [(u8, u32, u8)] {
    match total_coeff {
        1 => TOTAL_ZEROS_1,
        2 => TOTAL_ZEROS_2,
        3 => TOTAL_ZEROS_3,
        4 => TOTAL_ZEROS_4,
        5 => TOTAL_ZEROS_5,
        6 => TOTAL_ZEROS_6,
        7 => TOTAL_ZEROS_7,
        8 => TOTAL_ZEROS_8,
        9 => TOTAL_ZEROS_9,
        10 => TOTAL_ZEROS_10,
        11 => TOTAL_ZEROS_11,
        12 => TOTAL_ZEROS_12,
        13 => TOTAL_ZEROS_13,
        14 => TOTAL_ZEROS_14,
        15 => TOTAL_ZEROS_15,
        _ => TOTAL_ZEROS_15,
    }
}

/// Write total_zeros using the VLC table for the given total_coeff (Table 9-7).
fn write_total_zeros(w: &mut BitstreamWriter, total_coeff: u32, total_zeros: u32) {
    let table = get_total_zeros_table(total_coeff);
    for &(value, code, bits) in table {
        if value as u32 == total_zeros {
            w.write_bits(code, bits);
            return;
        }
    }
    // Fallback: should not happen with valid input
    w.write_bits(0, 1);
}

// Chroma DC total_zeros VLC tables (Table 9-9(a), 4:2:0). Format: (value, code, bits).
#[rustfmt::skip]
static TOTAL_ZEROS_CHROMA_DC_1: &[(u8, u32, u8)] = &[
    (0, 0b1, 1), (1, 0b01, 2), (2, 0b001, 3), (3, 0b000, 3),
];
#[rustfmt::skip]
static TOTAL_ZEROS_CHROMA_DC_2: &[(u8, u32, u8)] = &[
    (0, 0b1, 1), (1, 0b01, 2), (2, 0b00, 2),
];
#[rustfmt::skip]
static TOTAL_ZEROS_CHROMA_DC_3: &[(u8, u32, u8)] = &[
    (0, 0b1, 1), (1, 0b0, 1),
];

/// Write chroma-DC total_zeros using the VLC table for the given total_coeff (Table 9-9).
fn write_total_zeros_chroma_dc(w: &mut BitstreamWriter, total_coeff: u32, total_zeros: u32) {
    let table: &[(u8, u32, u8)] = match total_coeff {
        1 => TOTAL_ZEROS_CHROMA_DC_1,
        2 => TOTAL_ZEROS_CHROMA_DC_2,
        _ => TOTAL_ZEROS_CHROMA_DC_3,
    };
    for &(value, code, bits) in table {
        if value as u32 == total_zeros {
            w.write_bits(code, bits);
            return;
        }
    }
    w.write_bits(0, 1);
}

// ============================================================
// run_before VLC tables (H.264 Table 9-10)
// Format: (value, codeword, bit_length)
// ============================================================

#[rustfmt::skip]
static RUN_BEFORE_1: &[(u8, u32, u8)] = &[
    (0, 0b1, 1), (1, 0b0, 1),
];

#[rustfmt::skip]
static RUN_BEFORE_2: &[(u8, u32, u8)] = &[
    (0, 0b1, 1), (1, 0b01, 2), (2, 0b00, 2),
];

#[rustfmt::skip]
static RUN_BEFORE_3: &[(u8, u32, u8)] = &[
    (0, 0b11, 2), (1, 0b10, 2), (2, 0b01, 2), (3, 0b00, 2),
];

#[rustfmt::skip]
static RUN_BEFORE_4: &[(u8, u32, u8)] = &[
    (0, 0b11, 2), (1, 0b10, 2), (2, 0b01, 2), (3, 0b001, 3), (4, 0b000, 3),
];

#[rustfmt::skip]
static RUN_BEFORE_5: &[(u8, u32, u8)] = &[
    (0, 0b11, 2), (1, 0b10, 2), (2, 0b011, 3), (3, 0b010, 3),
    (4, 0b001, 3), (5, 0b000, 3),
];

#[rustfmt::skip]
static RUN_BEFORE_6: &[(u8, u32, u8)] = &[
    (0, 0b11, 2), (1, 0b000, 3), (2, 0b001, 3), (3, 0b011, 3),
    (4, 0b010, 3), (5, 0b101, 3), (6, 0b100, 3),
];

#[rustfmt::skip]
static RUN_BEFORE_7PLUS: &[(u8, u32, u8)] = &[
    (0,  0b111, 3), (1,  0b110, 3), (2,  0b101, 3), (3,  0b100, 3),
    (4,  0b011, 3), (5,  0b010, 3), (6,  0b001, 3),
    (7,  0b0001, 4), (8,  0b00001, 5), (9,  0b000001, 6),
    (10, 0b0000001, 7), (11, 0b00000001, 8), (12, 0b000000001, 9),
    (13, 0b0000000001, 10), (14, 0b00000000001, 11),
];

fn get_run_before_table(zeros_left: u32) -> &'static [(u8, u32, u8)] {
    match zeros_left {
        0 => &[],
        1 => RUN_BEFORE_1,
        2 => RUN_BEFORE_2,
        3 => RUN_BEFORE_3,
        4 => RUN_BEFORE_4,
        5 => RUN_BEFORE_5,
        6 => RUN_BEFORE_6,
        _ => RUN_BEFORE_7PLUS,
    }
}

/// Write run_before using the VLC table for the given zeros_left (Table 9-10).
fn write_run_before(w: &mut BitstreamWriter, zeros_left: u32, run: u32) {
    if zeros_left == 0 {
        return;
    }
    let table = get_run_before_table(zeros_left);
    let capped_run = run.min(table.len() as u32 - 1);
    for &(value, code, bits) in table {
        if value as u32 == capped_run {
            w.write_bits(code, bits);
            return;
        }
    }
    // Fallback
    w.write_bits(0, 1);
}

/// Write CAVLC residual block coefficients.
///
/// Implements the H.264 CAVLC residual coding (spec 9.2.3):
/// 1. Write TotalCoeffs and TrailingOnes token (context-dependent by nC)
/// 2. Write level values (for non-trailing coefficients)
/// 3. Write total_zeros
/// 4. Write run_before for each coefficient
///
/// nC: number of non-zero coefficients in neighboring blocks.
/// nC < 0 means chroma DC context.
#[allow(
    non_snake_case,
    reason = "nC mirrors the H.264 spec's own name for this value"
)]
pub fn write_residual_block(
    w: &mut BitstreamWriter,
    coefficients: &[i16],
    _block_type: BlockType,
    max_coeffs: u32,
    nC: i32,
) -> u32 {
    // Collect the non-zero coefficients in reverse scan order (highest frequency
    // first), recording for each one after the first the run of zeros separating it
    // from the previous (higher-frequency) non-zero coefficient. Zeros above the
    // highest-frequency coefficient are not part of total_zeros and are dropped.
    let mut coeff_vals: Vec<i16> = Vec::new();
    let mut runs: Vec<u32> = Vec::new();
    let last_nonzero = coefficients.iter().rposition(|&c| c != 0);

    if let Some(last) = last_nonzero {
        let mut zero_run = 0u32;
        for i in (0..=last).rev() {
            let c = coefficients[i];
            if c == 0 {
                zero_run += 1;
            } else {
                if !coeff_vals.is_empty() {
                    runs.push(zero_run);
                }
                coeff_vals.push(c);
                zero_run = 0;
            }
        }
    }

    let total_coeffs = coeff_vals.len() as u32;

    // TrailingOnes: the leading run of ±1 coefficients in the reverse-ordered list,
    // capped at 3 (H.264 9.2.2). Zeros between coefficients do NOT break this run —
    // the count is over the non-zero coefficient list, matching the decoder. This is
    // what guarantees the first non-trailing level has |level| >= 2 below.
    let mut trailing_ones = 0u32;
    for &c in &coeff_vals {
        if (c == 1 || c == -1) && trailing_ones < 3 {
            trailing_ones += 1;
        } else {
            break;
        }
    }

    // 1. TotalCoeffs / TrailingOnes token (VLC table selected by nC).
    write_coeff_token(w, nC, total_coeffs, trailing_ones);

    if total_coeffs == 0 {
        return 0;
    }

    // 1b. trailing_ones_sign_flag for each trailing one, highest frequency first.
    for &c in coeff_vals.iter().take(trailing_ones as usize) {
        w.write_bits(if c < 0 { 1 } else { 0 }, 1);
    }

    // 2. Levels for the non-trailing coefficients (still highest frequency first).
    let levels = &coeff_vals[trailing_ones as usize..];
    let mut suffix_length: u32 = if total_coeffs > 10 && trailing_ones < 3 {
        1
    } else {
        0
    };

    for (i, &level) in levels.iter().enumerate() {
        let abs_level = level.unsigned_abs() as u32;

        // level_code per H.264 9.2.2.1: even => positive, odd => negative.
        let mut level_code: u32 = if level > 0 {
            (abs_level - 1) << 1
        } else {
            (((-level) as u32) - 1) << 1 | 1
        };

        // The first non-trailing level, when TrailingOnes < 3, is biased by -2:
        // such a coefficient cannot be ±1 (it would have been a trailing one), so
        // abs_level >= 2 here and level_code >= 2 — no underflow.
        if i == 0 && trailing_ones < 3 {
            level_code -= 2;
        }

        write_level_code(w, level_code, suffix_length);

        // Adapt suffix_length per H.264 9.2.2.1 (mirrors the decoder exactly).
        if suffix_length == 0 {
            suffix_length = 1;
        }
        if abs_level > (3 << (suffix_length - 1)) && suffix_length < 6 {
            suffix_length += 1;
        }
    }

    // 3. total_zeros (count of zero coefficients below the highest-frequency one).
    let total_zeros = match last_nonzero {
        Some(last) => coefficients[..last].iter().filter(|&&c| c == 0).count() as u32,
        None => 0,
    };
    if total_coeffs < max_coeffs {
        // Chroma DC (max 4 coefficients) uses a distinct total_zeros table (Table 9-9).
        if max_coeffs <= 4 {
            write_total_zeros_chroma_dc(w, total_coeffs, total_zeros);
        } else {
            write_total_zeros(w, total_coeffs, total_zeros);
        }
    }

    // 4. run_before for every coefficient except the highest-frequency one, in
    //    decode order. The decoder stops reading once zeros_left reaches 0 (all later
    //    runs are then implicitly 0) and treats the lowest-frequency coefficient's
    //    run as the leftover, so neither is written here.
    let mut zeros_left = total_zeros;
    for &run in &runs {
        if zeros_left == 0 {
            break;
        }
        write_run_before(w, zeros_left, run);
        zeros_left -= run;
    }

    total_coeffs
}

/// Write one CAVLC level value from its `level_code` and the current `suffix_length`,
/// matching the decoder's `parse_level` exactly (H.264 9.2.2.1) — including the
/// level_prefix==14 (4-bit suffix) and level_prefix==15 (12-bit suffix) escape forms
/// needed for large coefficient magnitudes. Without these, a single large level emits
/// a runaway unary prefix that overflows the decoder's exp-golomb reader.
fn write_level_code(w: &mut BitstreamWriter, level_code: u32, suffix_length: u32) {
    if suffix_length == 0 {
        if level_code < 14 {
            write_unary_prefix(w, level_code);
        } else if level_code < 30 {
            write_unary_prefix(w, 14);
            w.write_bits(level_code - 14, 4);
        } else {
            // level_prefix = 15: decoder reconstructs level_code = 30 + suffix.
            write_unary_prefix(w, 15);
            w.write_bits((level_code - 30).min(0xFFF), 12);
        }
    } else {
        let threshold = 15u32 << suffix_length;
        if level_code < threshold {
            let level_prefix = level_code >> suffix_length;
            write_unary_prefix(w, level_prefix);
            w.write_bits(level_code & ((1 << suffix_length) - 1), suffix_length as u8);
        } else {
            // level_prefix = 15: decoder reconstructs level_code = (15 << sl) + suffix.
            write_unary_prefix(w, 15);
            w.write_bits((level_code - threshold).min(0xFFF), 12);
        }
    }
}

/// Write `prefix` zero bits followed by a single 1 bit (a level_prefix unary code).
fn write_unary_prefix(w: &mut BitstreamWriter, prefix: u32) {
    for _ in 0..prefix {
        w.write_bits(0, 1);
    }
    w.write_bits(1, 1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // H.264 Table 9-4 (I-slice column): code_number -> CBP byte. Mirror of the
    // table inside `find_cbp_code`, used here to independently verify the lookup.
    #[rustfmt::skip]
    const CBP_INTRA_TABLE: [u8; 48] = [
        47, 31, 15,  0, 23, 27, 29, 30,  7, 11, 13, 14, 39, 43, 45, 46,
        16,  3,  5, 10, 12, 19, 21, 26, 28, 35, 37, 42, 44,  1,  2,  4,
         8, 17, 18, 20, 24,  6,  9, 22, 25, 32, 33, 34, 36, 40, 38, 41,
    ];

    #[test]
    fn zero_block_produces_token() {
        let mut w = BitstreamWriter::new();
        write_residual_block(&mut w, &[0; 16], BlockType::Luma4x4, 16, 0);
        let bytes = w.take_bytes();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn single_coefficient() {
        let mut coeffs = [0i16; 16];
        coeffs[0] = 5;
        let mut w = BitstreamWriter::new();
        write_residual_block(&mut w, &coeffs, BlockType::Luma4x4, 16, 0);
        let bytes = w.take_bytes();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn zero_block_token_is_one_bit() {
        // coeff_token(total_coeff=0, trailing_ones=0) for nC<2 is the single bit `1`,
        // and the block returns immediately afterwards => one byte 0b1000_0000.
        let mut w = BitstreamWriter::new();
        write_residual_block(&mut w, &[0i16; 16], BlockType::Luma4x4, 16, 0);
        assert_eq!(w.take_bytes(), vec![0x80]);
    }

    #[test]
    fn find_cbp_code_golden() {
        assert_eq!(find_cbp_code(0, 0, true), 3); // CBP 0  -> code 3
        assert_eq!(find_cbp_code(15, 0, true), 2); // CBP 15 -> code 2
        assert_eq!(find_cbp_code(15, 2, true), 0); // CBP 47 -> code 0
    }

    proptest! {
        /// Every (cbp_luma, cbp_chroma) in range maps to a code that decodes back to it.
        #[test]
        fn find_cbp_code_roundtrips(cbp_luma in 0u32..16, cbp_chroma in 0u32..3) {
            let target = ((cbp_chroma << 4) | cbp_luma) as u8;
            let code = find_cbp_code(cbp_luma, cbp_chroma, true);
            prop_assert!((code as usize) < CBP_INTRA_TABLE.len());
            prop_assert_eq!(CBP_INTRA_TABLE[code as usize], target);
        }
    }
}
