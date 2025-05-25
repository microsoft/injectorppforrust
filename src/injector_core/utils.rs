#![cfg(target_arch = "aarch64")]

/// Convert a u64 value into a [bool; 64] array of bits.
/// Bit 0 is the least-significant bit.
pub fn u64_to_bits(n: u64) -> [bool; 64] {
    let mut bits = [false; 64];
    for (i, bit) in bits.iter_mut().enumerate() {
        *bit = ((n >> i) & 1) != 0;
    }
    bits
}

/// Converts an 8-bit number into an array of N booleans representing its bits.
/// The least-significant bit is at index 0.
pub fn u8_to_bits<const N: usize>(n: u8) -> [bool; N] {
    let mut bits = [false; N];
    for (i, bit) in bits.iter_mut().enumerate() {
        *bit = ((n >> i) & 1) != 0;
    }
    bits
}

pub fn bool_array_to_u32(bits: [bool; 32]) -> u32 {
    bits.iter()
        .enumerate()
        .fold(0, |acc, (i, &bit)| if bit { acc | (1 << i) } else { acc })
}
