//! AV1 symbol decoder (arithmetic coder).
//!
//! Implements the Daala entropy coder used by AV1.
//! Reference: AV1 spec section 6.2 (Symbol decoding process)
//! Reference: AV1 spec section 7.2 (Entropy coding process)

/// AV1 range decoder state.
///
/// The range decoder maintains a current range and value, and reads
/// bits from the bitstream to decode symbols based on CDF probabilities.
#[derive(Debug)]
pub struct SymbolDecoder {
    /// Current range (initialized to 32768).
    range: u32,
    /// Current value (bits read from bitstream).
    value: u32,
    /// Number of bits consumed.
    bits_consumed: u32,
}

impl SymbolDecoder {
    /// Create a new symbol decoder from a bitstream.
    ///
    /// The first 16 bits are read to initialize the decoder state.
    pub fn new(data: &[u8]) -> Result<Self, String> {
        if data.len() < 2 {
            return Err("bitstream too short for symbol decoder".into());
        }

        // Read initial 16 bits
        let value = ((data[0] as u32) << 8) | (data[1] as u32);

        Ok(Self {
            range: 32768,
            value,
            bits_consumed: 16,
        })
    }

    /// Decode a binary symbol with given probability (0..32768).
    ///
    /// `prob` is the probability of the symbol being 0 (out of 32768).
    /// Returns the decoded symbol (0 or 1).
    pub fn read_bit(&mut self, prob: u32, data: &[u8]) -> Result<u8, String> {
        let range = self.range;
        let value = self.value;

        // Split the range based on probability
        let split = ((range as u64 * prob as u64) >> 15) as u32;

        let symbol = if value >> 15 >= split {
            // Symbol is 1
            self.value = value - (split << 15);
            self.range = (range - split) << 1;
            1
        } else {
            // Symbol is 0
            self.value = value << 15;
            self.range = split << 1;
            0
        };

        // Renormalize: if range < 32768, read more bits
        while self.range < 32768 {
            self.range <<= 1;
            self.value <<= 1;
            self.bits_consumed += 1;

            // Read next bit from bitstream
            let byte_idx = (self.bits_consumed / 8) as usize;
            let bit_idx = 7 - (self.bits_consumed % 8);
            if byte_idx < data.len() {
                let bit = (data[byte_idx] >> bit_idx) & 1;
                self.value |= bit as u32;
            }
        }

        Ok(symbol)
    }

    /// Decode a symbol using a CDF (cumulative distribution function).
    ///
    /// `cdf` is the CDF table (values 0..32768, last entry is 0).
    /// Returns the decoded symbol index.
    pub fn read_symbol(&mut self, cdf: &[u32], data: &[u8]) -> Result<u32, String> {
        if cdf.is_empty() {
            return Ok(0);
        }

        let n_symbols = cdf.len();
        let mut symbol = 0u32;

        // Binary search through the CDF
        for i in 0..n_symbols - 1 {
            let prob = cdf[i];
            let bit = self.read_bit(prob, data)?;
            if bit == 0 {
                symbol = i as u32;
                break;
            }
            symbol = (i + 1) as u32;
        }

        Ok(symbol)
    }

    /// Decode a symbol using a u16 CDF (as used by AV1).
    ///
    /// `cdf` is the CDF table (u16 values 0..32768, last entry should be 32768).
    /// Returns the decoded symbol index.
    pub fn read_symbol_from_cdf(&mut self, cdf: &[u16], data: &[u8]) -> Result<u8, String> {
        if cdf.is_empty() {
            return Ok(0);
        }

        let n_symbols = cdf.len();
        let mut symbol = 0u8;

        for i in 0..n_symbols - 1 {
            let prob = cdf[i] as u32;
            let bit = self.read_bit(prob, data)?;
            if bit == 0 {
                symbol = i as u8;
                break;
            }
            symbol = (i + 1) as u8;
        }

        Ok(symbol)
    }

    /// Decode an unsigned integer with given number of bits.
    pub fn read_literal(&mut self, n_bits: u32, data: &[u8]) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..n_bits {
            let bit = self.read_bit(16384, data)?; // 50% probability
            value = (value << 1) | bit as u32;
        }
        Ok(value)
    }

    /// Get the number of bits consumed so far.
    pub fn bits_consumed(&self) -> u32 {
        self.bits_consumed
    }
}

/// CDF update process (AV1 spec 7.2.3).
///
/// Updates the CDF based on the decoded symbol.
/// A lower CDF value means higher probability for that symbol.
pub fn update_cdf(cdf: &mut [u32], symbol: u32, n_symbols: usize) {
    let rate = 4; // simplified adaptation rate

    for i in 0..n_symbols - 1 {
        if i < symbol as usize {
            // Increase cdf (decrease probability for symbols below decoded)
            cdf[i] = cdf[i] + ((32768 - cdf[i]) >> rate);
        } else {
            // Decrease cdf (increase probability for symbols at or above decoded)
            cdf[i] = cdf[i].saturating_sub(cdf[i] >> rate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_decoder_new() {
        let data = [0x80, 0x00]; // 10000000 00000000
        let dec = SymbolDecoder::new(&data);
        assert!(dec.is_ok());
    }

    #[test]
    fn symbol_decoder_short_data() {
        let data = [0x80];
        let dec = SymbolDecoder::new(&data);
        assert!(dec.is_err());
    }

    #[test]
    fn read_bit_deterministic() {
        // With probability 0 (always 1) and value 0x8000 (bit 15 set),
        // the first bit should be 1
        let data = [0x80, 0x00];
        let mut dec = SymbolDecoder::new(&data).unwrap();
        // prob=0 means split=0, so value>>15 (1) >= split (0) → symbol=1
        let bit = dec.read_bit(0, &data).unwrap();
        assert_eq!(bit, 1);
    }

    #[test]
    fn update_cdf_basic() {
        let mut cdf = vec![16384, 0]; // 50% probability
        update_cdf(&mut cdf, 0, 2);
        // After decoding symbol 0, cdf[0] should decrease
        assert!(cdf[0] < 16384);
    }

    #[test]
    fn update_cdf_symbol_1() {
        let mut cdf = vec![16384, 0]; // 50% probability
        update_cdf(&mut cdf, 1, 2);
        // After decoding symbol 1, cdf[0] should increase
        assert!(cdf[0] > 16384);
    }
}
