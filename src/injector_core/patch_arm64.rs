#![cfg(target_arch = "aarch64")]

use crate::injector_core::arm64_codegenerator::*;
use crate::injector_core::common::*;
use crate::injector_core::patch_trait::*;
use crate::injector_core::utils::*;

pub(crate) struct PatchArm64;

impl PatchTrait for PatchArm64 {
    fn replace_function_with_other_function(src: *mut u8, target: *const ()) -> PatchGuard {
        let patch_size = 12;
        let original_bytes = unsafe { read_bytes(src, patch_size) };
        let jit_size = 20;
        let jit_memory = allocate_jit_memory(src, jit_size);
        generate_will_execute_jit_code_abs(jit_memory, target);
        let func_addr = src as usize;
        let jit_addr = jit_memory as usize;
        let offset = (jit_addr as isize - func_addr as isize) / 4;
        if !(-33554432..=33554431).contains(&offset) {
            panic!("JIT memory is out of branch range");
        }
        let branch_instr: u32 = 0x14000000 | ((offset as u32) & 0x03ffffff);
        let nop: u32 = 0xd503201f;
        let mut patch = [0u8; 12];
        patch[0..4].copy_from_slice(&branch_instr.to_le_bytes());
        patch[4..8].copy_from_slice(&nop.to_le_bytes());
        patch[8..12].copy_from_slice(&nop.to_le_bytes());
        unsafe {
            patch_function(src, &patch);
        }
        PatchGuard {
            func_ptr: src,
            original_bytes,
            patch_size,
            jit_memory,
            jit_size,
        }
    }

    fn replace_function_return_boolean(src: *mut u8, value: bool) -> PatchGuard {
        let patch_size = 12;
        let original_bytes = unsafe { read_bytes(src, patch_size) };
        let jit_size = 8;
        let jit_memory = allocate_jit_memory(src, jit_size);
        generate_will_return_boolean_jit_code(jit_memory, value);
        let func_addr = src as usize;
        let jit_addr = jit_memory as usize;
        let offset = (jit_addr as isize - func_addr as isize) / 4;
        if !(-33554432..=33554431).contains(&offset) {
            panic!("JIT memory is out of branch range");
        }
        let branch_instr: u32 = 0x14000000 | ((offset as u32) & 0x03ffffff);
        let nop: u32 = 0xd503201f;
        let mut patch = [0u8; 12];
        patch[0..4].copy_from_slice(&branch_instr.to_le_bytes());
        patch[4..8].copy_from_slice(&nop.to_le_bytes());
        patch[8..12].copy_from_slice(&nop.to_le_bytes());
        unsafe {
            patch_function(src, &patch);
        }
        PatchGuard {
            func_ptr: src,
            original_bytes,
            patch_size,
            jit_memory,
            jit_size,
        }
    }
}

/// Generates a 16-byte JIT code block that loads the absolute address of `target`
/// into register X9 (using a MOVZ and two MOVK instructions) and then branches to X9.
/// This avoids branch-range limitations.
///
/// The generated instructions are:
///   movz x9, #imm0, lsl #0
///   movk x9, #imm1, lsl #16
///   movk x9, #imm2, lsl #32
///   br x9
fn generate_will_execute_jit_code_abs(jit_ptr: *mut u8, target: *const ()) {
    let target_addr = target as usize as u64;

    // x9
    let register_name: [bool; 5] = u8_to_bits::<5>(9);

    // MOVZ x9, #imm0 (clears the rest)
    let movz = emit_movz_from_address(target_addr, 0, true, u8_to_bits::<2>(0), register_name);

    // MOVK x9, #imm1, LSL #16
    let movk1 = emit_movk_from_address(target_addr, 16, true, u8_to_bits::<2>(1), register_name);

    // MOVK x9, #imm2, LSL #32
    let movk2 = emit_movk_from_address(target_addr, 32, true, u8_to_bits::<2>(2), register_name);

    // MOVK x9, #imm3, LSL #48
    let movk3 = emit_movk_from_address(target_addr, 48, true, u8_to_bits::<2>(3), register_name);

    // BR x9
    let br = emit_br(register_name);

    // Write instructions in the correct order: bottom-up so no overwrite
    let mut asm_code: Vec<u8> = Vec::new();
    append_instruction(&mut asm_code, bool_array_to_u32(movz));
    append_instruction(&mut asm_code, bool_array_to_u32(movk1));
    append_instruction(&mut asm_code, bool_array_to_u32(movk2));
    append_instruction(&mut asm_code, bool_array_to_u32(movk3));
    append_instruction(&mut asm_code, bool_array_to_u32(br));

    unsafe {
        inject_asm_code(&asm_code, jit_ptr);
    }
}

/// Generates a 16-byte JIT code block that returns the specified boolean.
/// The code moves the immediate into w0 and then returns.
/// Two NOPs are added for padding.
fn generate_will_return_boolean_jit_code(jit_ptr: *mut u8, value: bool) {
    let mut asm_code: Vec<u8> = Vec::new();

    let mut value_bits = [false; 16];
    value_bits[0] = value;

    let movz = emit_movz(value_bits, true, u8_to_bits::<2>(0), u8_to_bits::<5>(0));

    let ret = emit_ret_x30();

    append_instruction(&mut asm_code, bool_array_to_u32(movz));
    append_instruction(&mut asm_code, bool_array_to_u32(ret));

    unsafe {
        inject_asm_code(&asm_code, jit_ptr);
    }
}

fn append_instruction(asm_code: &mut Vec<u8>, instruction: u32) {
    asm_code.push((instruction & 0xFF) as u8);
    asm_code.push(((instruction >> 8) & 0xFF) as u8);
    asm_code.push(((instruction >> 16) & 0xFF) as u8);
    asm_code.push(((instruction >> 24) & 0xFF) as u8);
}
