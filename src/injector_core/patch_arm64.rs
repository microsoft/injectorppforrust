#![cfg(target_arch = "aarch64")]

use crate::injector_core::arm64_codegenerator::*;
use crate::injector_core::common::*;
use crate::injector_core::patch_trait::*;
use crate::injector_core::utils::*;

pub(crate) struct PatchArm64;

impl PatchTrait for PatchArm64 {
    fn replace_function_with_other_function(
        src: FuncPtrInternal,
        target: FuncPtrInternal,
    ) -> PatchGuard {
        const PATCH_SIZE: usize = 12;
        const JIT_SIZE: usize = 20;

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            if let Some(size) = get_function_size(src.as_ptr()) {
                if size == 0 {
                    panic!(
                "Function at address {:?} has st_size == 0 (unknown size). Refusing to patch.",
                src.as_ptr()
            );
                }
                if size < PATCH_SIZE {
                    panic!(
                        "Function at address {:?} is too small ({} bytes). Required: {} bytes.",
                        src.as_ptr(),
                        size,
                        PATCH_SIZE
                    );
                }
            } else {
                panic!(
                    "Unable to determine function size for {:?}; refusing to patch.",
                    src.as_ptr()
                );
            }
        }

        let original_bytes = unsafe { read_bytes(src.as_ptr() as *mut u8, PATCH_SIZE) };
        let jit_memory = allocate_jit_memory(&src, JIT_SIZE);
        generate_will_execute_jit_code_abs(jit_memory, target.as_ptr());

        apply_branch_patch(src, jit_memory, JIT_SIZE, &original_bytes)
    }

    fn replace_function_return_boolean(src: FuncPtrInternal, value: bool) -> PatchGuard {
        const PATCH_SIZE: usize = 12;
        const JIT_SIZE: usize = 8;

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            if let Some(size) = get_function_size(src.as_ptr()) {
                if size == 0 {
                    panic!(
                "Function at address {:?} has st_size == 0 (unknown size). Refusing to patch.",
                src.as_ptr()
            );
                }
                if size < PATCH_SIZE {
                    panic!(
                        "Function at address {:?} is too small ({} bytes). Required: {} bytes.",
                        src.as_ptr(),
                        size,
                        PATCH_SIZE
                    );
                }
            } else {
                panic!(
                    "Unable to determine function size for {:?}; refusing to patch.",
                    src.as_ptr()
                );
            }
        }

        let original_bytes = unsafe { read_bytes(src.as_ptr() as *mut u8, PATCH_SIZE) };
        let jit_memory = allocate_jit_memory(&src, JIT_SIZE);
        generate_will_return_boolean_jit_code(jit_memory, value);

        apply_branch_patch(src, jit_memory, JIT_SIZE, &original_bytes)
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
    let mut asm_code = [0u8; 8]; // 2 instructions = 2 * 4
    let mut cursor = 0;

    let mut value_bits = [false; 16];
    value_bits[0] = value;

    let movz = emit_movz(value_bits, true, u8_to_bits::<2>(0), u8_to_bits::<5>(0));
    let ret = emit_ret_x30();

    write_instruction(&mut asm_code, &mut cursor, bool_array_to_u32(movz));
    write_instruction(&mut asm_code, &mut cursor, bool_array_to_u32(ret));

    unsafe {
        inject_asm_code(&asm_code, jit_ptr);
    }
}

#[inline]
fn write_instruction(buf: &mut [u8], cursor: &mut usize, instruction: u32) {
    let bytes = instruction.to_le_bytes();
    buf[*cursor..*cursor + 4].copy_from_slice(&bytes);
    *cursor += 4;
}

fn append_instruction(asm_code: &mut Vec<u8>, instruction: u32) {
    asm_code.push((instruction & 0xFF) as u8);
    asm_code.push(((instruction >> 8) & 0xFF) as u8);
    asm_code.push(((instruction >> 16) & 0xFF) as u8);
    asm_code.push(((instruction >> 24) & 0xFF) as u8);
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[inline]
fn get_function_size(ptr: *const ()) -> Option<usize> {
    use libc::{c_int, c_void, Dl_info};

    const RTLD_DI_SYMENT: c_int = 2;

    unsafe {
        let mut info: Dl_info = std::mem::zeroed();
        if libc::dladdr(ptr as *const c_void, &mut info) == 0 {
            return None;
        }

        let mut sym_ptr: *const libc::Elf64_Sym = std::ptr::null();
        let result = libc::dlinfo(
            info.dli_fbase as *mut c_void,
            RTLD_DI_SYMENT,
            &mut sym_ptr as *mut _ as *mut c_void,
        );

        if result != 0 || sym_ptr.is_null() {
            return None;
        }

        Some((*sym_ptr).st_size as usize)
    }
}

fn apply_branch_patch(
    src: FuncPtrInternal,
    jit_memory: *mut u8,
    jit_size: usize,
    original_bytes: &[u8],
) -> PatchGuard {
    const PATCH_SIZE: usize = 12;
    const BRANCH_RANGE: std::ops::RangeInclusive<isize> = -0x2000000..=0x1FFF_FFFF; // ±32MB
    const NOP: u32 = 0xd503201f;

    let func_addr = src.as_ptr() as usize;
    let jit_addr = jit_memory as usize;
    let offset = (jit_addr as isize - func_addr as isize) / 4;

    if !BRANCH_RANGE.contains(&offset) {
        panic!("JIT memory is out of branch range: offset = {offset}, expected ±32MB");
    }

    let branch_instr: u32 = 0x14000000 | ((offset as u32) & 0x03FF_FFFF);

    let mut patch = [0u8; PATCH_SIZE];
    patch[0..4].copy_from_slice(&branch_instr.to_le_bytes());
    patch[4..8].copy_from_slice(&NOP.to_le_bytes());
    patch[8..12].copy_from_slice(&NOP.to_le_bytes());

    unsafe {
        patch_function(src.as_ptr() as *mut u8, &patch);
    }

    PatchGuard::new(
        src.as_ptr() as *mut u8,
        original_bytes.to_vec(),
        PATCH_SIZE,
        jit_memory,
        jit_size,
    )
}
