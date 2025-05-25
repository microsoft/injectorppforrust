#![cfg(target_arch = "x86_64")]

use crate::injector_core::common::*;
use crate::injector_core::patch_trait::*;

pub struct PatchAmd64;

impl PatchTrait for PatchAmd64 {
    fn replace_function_with_other_function(src: *mut u8, target: *const ()) -> PatchGuard {
        // The code size is maximum 12 bytes because only a jmp instruction is needed.
        let jit_size = 12;
        let jit_memory = allocate_jit_memory(src, jit_size);

        let target_addr = target as usize;
        let jit_addr = jit_memory as usize;

        // The jit code is simply jumping to the target address.
        let jit_code = generate_branch_to_target_function(jit_addr, target_addr);

        // Write the jit code to the jit memory.
        unsafe {
            inject_asm_code(&jit_code, jit_memory);
        }

        // Now modify the original function to branch to the jit memory
        let func_addr = src as usize;

        let branch_code = generate_branch_to_target_function(func_addr, jit_addr);

        let patch_size = branch_code.len();
        let original_bytes = unsafe { read_bytes(src, patch_size) };

        unsafe {
            patch_function(src, &branch_code);
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
        let jit_size = 8;
        let jit_memory = allocate_jit_memory(src, jit_size);

        generate_will_return_boolean_jit_code(jit_memory, value);

        let func_addr = src as usize;
        let jit_addr = jit_memory as usize;

        let branch_code = generate_branch_to_target_function(func_addr, jit_addr);

        let patch_size = branch_code.len();
        let original_bytes = unsafe { read_bytes(src, patch_size) };

        unsafe {
            patch_function(src, &branch_code);
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

fn generate_will_return_boolean_jit_code(jit_ptr: *mut u8, value: bool) {
    let mut asm_code: Vec<u8> = vec![
        // mov rax, 0x00;
        // ret;
        0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00, 0xC3,
    ];

    // Replace the value accordingly
    if value == true {
        asm_code[3] = 1u8;
    }

    unsafe {
        inject_asm_code(&asm_code, jit_ptr);
    }
}

fn generate_branch_to_target_function(ori_func: usize, target_func: usize) -> Vec<u8> {
    let mut branch_code: Vec<u8> = Vec::new();

    // +5 for 1-byte opcode + 4-byte offset
    let offset = target_func as isize - (ori_func as isize + 5);

    if offset <= i32::MAX as isize && offset >= i32::MIN as isize {
        // Use the 32-bit relative JMP
        branch_code.push(0xE9);
        let mut offset_u32 = offset as u32;
        for _ in 0..4 {
            branch_code.push((offset_u32 & 0xFF) as u8);
            offset_u32 >>= 8;
        }
    } else {
        let mut target = target_func;

        // mov rax, targetFunc
        branch_code.push(0x48);
        branch_code.push(0xB8);
        for _ in 0..8 {
            branch_code.push((target & 0xFF) as u8);
            target >>= 8;
        }

        // jmp rax
        branch_code.push(0xFF);
        branch_code.push(0xE0);
    }

    branch_code
}
