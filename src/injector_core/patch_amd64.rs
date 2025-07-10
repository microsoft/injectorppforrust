#![cfg(target_arch = "x86_64")]

use crate::injector_core::common::*;
use crate::injector_core::patch_trait::*;

/// Patch implementation for AMD64 (x86_64) architecture.
pub(crate) struct PatchAmd64;

/// Opcode constants for AMD64 jump and move instructions.
const JMP_REL_OPCODE: u8 = 0xE9;
const MOV_RAX_OPCODE: [u8; 2] = [0x48, 0xB8];
const JMP_RAX_OPCODE: [u8; 2] = [0xFF, 0xE0];

impl PatchTrait for PatchAmd64 {
    fn replace_function_with_other_function(
        src: FuncPtrInternal,
        target: FuncPtrInternal,
    ) -> PatchGuard {
        const JIT_SIZE: usize = 12;
        let jit_memory = allocate_jit_memory(&src, JIT_SIZE);

        let target_addr = target.as_ptr() as usize;
        let jit_addr = jit_memory as usize;

        let jit_code = generate_branch_to_target_function(jit_addr, target_addr);

        unsafe {
            inject_asm_code(&jit_code, jit_memory);
        }

        patch_and_guard(src, jit_memory, JIT_SIZE)
    }

    fn replace_function_return_boolean(src: FuncPtrInternal, value: bool) -> PatchGuard {
        const JIT_SIZE: usize = 8;
        let jit_memory = allocate_jit_memory(&src, JIT_SIZE);

        generate_will_return_boolean_jit_code(jit_memory, value);

        patch_and_guard(src, jit_memory, JIT_SIZE)
    }
}

/// Injects a return-boolean JIT sequence at `jit_ptr`.
fn generate_will_return_boolean_jit_code(jit_ptr: *mut u8, value: bool) {
    let mut asm_code: [u8; 8] = [
        0x48, 0xC7, 0xC0, // mov rax, imm32
        0x00, 0x00, 0x00, 0x00, // imm32
        0xC3, // ret
    ];

    asm_code[3] = value as u8;

    unsafe {
        inject_asm_code(&asm_code, jit_ptr);
    }
}

/// Generates a jump from `ori_func` to `target_func`.
fn generate_branch_to_target_function(ori_func: usize, target_func: usize) -> Vec<u8> {
    let offset = target_func as isize - (ori_func as isize + 5);

    if offset >= i32::MIN as isize && offset <= i32::MAX as isize {
        let mut branch_code = Vec::with_capacity(5);
        branch_code.push(JMP_REL_OPCODE);
        branch_code.extend_from_slice(&(offset as i32).to_le_bytes());
        branch_code
    } else {
        let mut branch_code = Vec::with_capacity(13);
        branch_code.extend_from_slice(&MOV_RAX_OPCODE);
        branch_code.extend_from_slice(&(target_func as u64).to_le_bytes());
        branch_code.extend_from_slice(&JMP_RAX_OPCODE);
        branch_code
    }
}

fn patch_and_guard(src: FuncPtrInternal, jit_memory: *mut u8, jit_size: usize) -> PatchGuard {
    let func_addr = src.as_ptr() as usize;
    let jit_addr = jit_memory as usize;

    let branch_code = generate_branch_to_target_function(func_addr, jit_addr);
    let patch_size = branch_code.len();

    let original_bytes = unsafe { read_bytes(src.as_ptr() as *mut u8, patch_size) };

    unsafe {
        patch_function(src.as_ptr() as *mut u8, &branch_code);
    }

    PatchGuard::new(
        src.as_ptr() as *mut u8,
        original_bytes,
        patch_size,
        jit_memory,
        jit_size,
    )
}
