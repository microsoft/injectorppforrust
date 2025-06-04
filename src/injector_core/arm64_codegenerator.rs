#![cfg(target_arch = "aarch64")]

use crate::injector_core::utils::*;

/// Insert `value` into a bit field of `u32` with range [lsb..=msb].
#[inline(always)]
const fn set_bits(value: u32, lsb: u8, msb: u8) -> u32 {
    debug_assert!(msb < 32 && lsb <= msb);
    debug_assert!(value < (1 << (msb - lsb + 1)));
    value << lsb
}

/// Extracts a u16 from `start` bit of a 64-bit value.
#[inline(always)]
const fn extract16(src: u64, start: usize) -> u16 {
    ((src >> start) & 0xFFFF) as u16
}
// C6.2.220 RET
// Return from subroutine branches unconditionally to an address in a register, with a hint that this is a subroutine return.
// x30 is used to hold the address to be branched to.
pub fn emit_ret_x30() -> [bool; 32] {
    emit_ret(&u8_to_bits::<5>(30))
}

// C6.2.220 RET
// Return from subroutine branches unconditionally to an address in a register,
// with a hint that this is a subroutine return.
pub fn emit_ret(register_name: &[bool; 5]) -> [bool; 32] {
    let reg_bits = bits_to_u8(register_name);
    let insn = 0b11010110010111100000000000000000u32 | set_bits(reg_bits as u32, 5, 9);
    u32_to_bits(insn)
}

/// Emit a 32‑bit BR (Branch to Register) instruction from a 5‑bit register name.
///
/// The instruction is built by concatenating fixed bit fields and the provided
/// register bits in the following order:
///
/// 1. 5 bits: 0,0,0,0,0  
/// 2. 5 bits: register_name  
/// 3. 2 bits: 0,0  
/// 4. 4 bits: 0,0,0,0
/// 5. 5 bits: 1,1,1,1,1  
/// 6. 2 bits: 0,0  
/// 7. 1 bit: 0  
/// 8. 1 bit: 0  
/// 9. 5 bits: 1,1,0,1,0  
///
/// Total: 5 + 5 + 2 + 6 + 5 + 2 + 1 + 1 + 5 = 32 bits.
pub fn emit_br(register_name: [bool; 5]) -> [bool; 32] {
    let reg_bits = bits_to_u8(&register_name);
    let insn = 0b11010110000111110000000000000000u32 | set_bits(reg_bits as u32, 5, 9);
    u32_to_bits(insn)
}

/// Converts a 64-bit address into a 32-bit instruction encoding.
/// It extracts 16 bits starting at `start` from the 64-bit address,
/// then calls `emit_movk` to build the final 32-bit code.
///
/// # Parameters
/// - `address`: The 64-bit address value.
/// - `start`: The starting bit index from which to extract 16 bits.
/// - `sf`: A flag bit.
/// - `hw`: A 2-bit value, represented as a [bool; 2].
/// - `register_name`: A 5-bit value, represented as a [bool; 5].
///
/// # Returns
/// A 32-bit code represented as a [bool; 32].
pub fn emit_movk_from_address(
    address: u64,
    start: usize,
    sf: bool,
    hw: [bool; 2],
    register_name: [bool; 5],
) -> [bool; 32] {
    let imm16 = extract16(address, start);
    emit_movk(u16_to_bits(imm16), sf, hw, register_name)
}

/// Builds the 32-bit instruction encoding by concatenating:
/// 1. The 5-bit register name.
/// 2. The 16-bit immediate value (`value_bits`).
/// 3. The 2-bit `hw` value.
/// 4. Fixed bits: 1,0,1,0,0,1 then 1,1.
/// 5. Finally the `sf` bit.
///
/// The total bit-length is 5 + 16 + 2 + 6 + 2 + 1 = 32 bits.
///
/// # Parameters
/// - `value_bits`: A 16-bit immediate value as [bool; 16].
/// - `sf`: A flag bit.
/// - `hw`: A 2-bit value as [bool; 2].
/// - `register_name`: A 5-bit value as [bool; 5].
///
/// # Returns
/// A 32-bit code represented as a [bool; 32].
pub fn emit_movk(
    value_bits: [bool; 16],
    sf: bool,
    hw: [bool; 2],
    register_name: [bool; 5],
) -> [bool; 32] {
    let rd = bits_to_u8(&register_name) as u32;
    let imm = bits_to_u16(&value_bits) as u32;
    let hw_val = bits_to_u8(&hw) as u32;

    let insn = (sf as u32) << 31
        | set_bits(0b111101, 25, 30) // opc
        | set_bits(hw_val, 21, 22)
        | set_bits(imm, 5, 20)
        | set_bits(0b101001, 10, 15)
        | set_bits(0b11, 23, 24)
        | set_bits(rd, 0, 4);

    u32_to_bits(insn)
}

/// Extracts a 16-bit immediate value from `address` starting at bit `start`
/// and then builds the final 32-bit MOVZ instruction.
///
/// # Parameters
/// - `address`: The 64-bit address value.
/// - `start`: The starting bit index for extraction.
/// - `sf`: A flag bit.
/// - `hw`: A 2-bit value as a [bool; 2].
/// - `register_name`: A 5-bit register name as a [bool; 5].
///
/// # Returns
/// A 32-bit code represented as a [bool; 32].
pub fn emit_movz_from_address(
    address: u64,
    start: usize,
    sf: bool,
    hw: [bool; 2],
    register_name: [bool; 5],
) -> [bool; 32] {
    let imm16 = extract16(address, start);
    emit_movz(u16_to_bits(imm16), sf, hw, register_name)
}

/// Assembles a 32-bit MOVZ instruction by concatenating:
/// 1. The 5-bit register name.
/// 2. The 16-bit immediate value (`value_bits`).
/// 3. The 2-bit hardware field (`hw`).
/// 4. Fixed bits: 1,0,1,0,0,1 followed by 0,1.
/// 5. Finally, the `sf` bit.
///
/// The bit ordering is maintained so that the final instruction is 32 bits long.
///
/// # Parameters
/// - `value_bits`: A 16-bit immediate value as a [bool; 16].
/// - `sf`: A flag bit.
/// - `hw`: A 2-bit value as a [bool; 2].
/// - `register_name`: A 5-bit register name as a [bool; 5].
///
/// # Returns
/// A 32-bit instruction encoded as a [bool; 32].
pub fn emit_movz(
    value_bits: [bool; 16],
    sf: bool,
    hw: [bool; 2],
    register_name: [bool; 5],
) -> [bool; 32] {
    let rd = bits_to_u8(&register_name) as u32;
    let imm = bits_to_u16(&value_bits) as u32;
    let hw_val = bits_to_u8(&hw) as u32;

    let insn = (sf as u32) << 31
        | set_bits(0b110101, 25, 30)
        | set_bits(hw_val, 21, 22)
        | set_bits(imm, 5, 20)
        | set_bits(0b101001, 10, 15)
        | set_bits(0b01, 23, 24)
        | set_bits(rd, 0, 4);

    u32_to_bits(insn)
}



pub fn u32_to_bits(v: u32) -> [bool; 32] {
    let mut out = [false; 32];
    for i in 0..32 {
        out[31 - i] = (v >> i) & 1 != 0;
    }
    out
}

pub fn u16_to_bits(v: u16) -> [bool; 16] {
    let mut out = [false; 16];
    for i in 0..16 {
        out[15 - i] = (v >> i) & 1 != 0;
    }
    out
}

pub fn bits_to_u16(bits: &[bool; 16]) -> u16 {
    bits.iter()
        .rev()
        .enumerate()
        .fold(0, |acc, (i, b)| acc | ((*b as u16) << i))
}

pub fn bits_to_u8(bits: &[bool; 5]) -> u8 {
    bits.iter()
        .rev()
        .enumerate()
        .fold(0, |acc, (i, b)| acc | ((*b as u8) << i))
}
