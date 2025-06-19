#![cfg(target_arch = "x86_64")]

use crate::injector_core::common::*;
use crate::injector_core::patch_trait::*;

pub(crate) struct PatchAmd64;

const JMP_REL_OPCODE: u8 = 0xE9;
const MOV_RAX_OPCODE: [u8; 2] = [0x48, 0xB8];
const JMP_RAX_OPCODE: [u8; 2] = [0xFF, 0xE0];

impl PatchTrait for PatchAmd64 {
    fn replace_function_with_other_function(
        src: FuncPtrInternal,
        target: FuncPtrInternal,
    ) -> PatchGuard {
        // The code size is maximum 12 bytes because only a jmp instruction is needed.
        let jit_size = 12;
        let jit_memory = allocate_jit_memory(&src, jit_size);

        let target_addr = target.as_ptr() as usize;
        let jit_addr = jit_memory as usize;

        // The jit code is simply jumping to the target address.
        let jit_code = generate_branch_to_target_function(jit_addr, target_addr);

        // Write the jit code to the jit memory.
        unsafe {
            inject_asm_code(&jit_code, jit_memory);
        }

        // Now modify the original function to branch to the jit memory
        let func_addr = src.as_ptr() as usize;

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

    fn replace_function_return_boolean(src: FuncPtrInternal, value: bool) -> PatchGuard {
        let jit_size = 8;
        let jit_memory = allocate_jit_memory(&src, jit_size);

        generate_will_return_boolean_jit_code(jit_memory, value);

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
}

fn generate_will_return_boolean_jit_code(jit_ptr: *mut u8, value: bool) {
    let mut asm_code: Vec<u8> = vec![
        // mov rax, 0x00;
        // ret;
        0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00, 0xC3,
    ];

    // Replace the value accordingly
    if value {
        asm_code[3] = 1u8;
    }

    unsafe {
        inject_asm_code(&asm_code, jit_ptr);
    }
}

fn generate_branch_to_target_function(ori_func: usize, target_func: usize) -> Vec<u8> {
    let offset = target_func as isize - (ori_func as isize + 5);

    if offset >= i32::MIN as isize && offset <= i32::MAX as isize {
        // Emit: jmp rel32 (5 bytes)
        let mut branch_code = Vec::with_capacity(5);
        branch_code.push(JMP_REL_OPCODE);
        branch_code.extend_from_slice(&(offset as i32).to_le_bytes());
        branch_code
    } else {
        // Emit: mov rax, imm64 + jmp rax (13 bytes)
        let mut branch_code = Vec::with_capacity(13);
        branch_code.extend_from_slice(&MOV_RAX_OPCODE);
        branch_code.extend_from_slice(&(target_func as u64).to_le_bytes());
        branch_code.extend_from_slice(&JMP_RAX_OPCODE);
        branch_code
    }
}
