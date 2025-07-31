#![cfg(target_arch = "arm")]

use std::ptr::null_mut;
use std::ptr::NonNull;

use crate::injector_core::common::*;
use crate::injector_core::patch_trait::*;

pub(crate) struct PatchArm;

impl PatchTrait for PatchArm {
    fn replace_function_with_other_function(
        src: FuncPtrInternal,
        target: FuncPtrInternal,
    ) -> PatchGuard {
        let is_src_thumb = src.as_ptr() as usize & 1 != 0;

        let src_ptr = if is_src_thumb {
            (src.as_ptr() as u32 - 1) as *const ()
        } else {
            src.as_ptr()
        };

        let patch_size = 12;
        let original_bytes = unsafe { read_bytes(src_ptr as *mut u8, patch_size) };

        let instructions: [u32; 3] = if is_src_thumb {
            [
                // ldr r7, [pc, #0] ; 0x4F00. It will load pc + 0 into r6, so the target word
                // bx r7 ; 4738
                // Reversed because of little endian
                0x47384F00,
                // .word target
                target.as_ptr() as u32,
                // .word anything (unused)
                0x00000000,
            ]
        } else {
            [
                // ldr r9, [pc, #-0] ; Load pc + 8 into r9, so the target word
                0xE51F9000,
                // bx r9 ; Branch to the target function
                0xE12FFF19,
                // .word target
                target.as_ptr() as u32,
            ]
        };

        let mut patch = [0u8; 12];

        patch[0..4].copy_from_slice(&instructions[0].to_le_bytes());
        patch[4..8].copy_from_slice(&instructions[1].to_le_bytes());
        patch[8..12].copy_from_slice(&instructions[2].to_le_bytes());

        // In thumb mode, if the source is not aligned on 32 bit, add a NOP to align it, so the target adress is also aligned on 32 bit
        // If we don't do that, the load adress will be misaligned and will load the bx instruction instead of the target function.
        if is_src_thumb && (src_ptr as usize % 4 != 0) {
            patch.rotate_right(2);
            patch[0] = 0xC0;
            patch[1] = 0x46; // NOP instruction in Thumb mode
        }

        unsafe {
            patch_function(src_ptr as *mut u8, &patch);
        }

        PatchGuard::new(
            src_ptr as *mut u8,
            original_bytes,
            patch_size,
            null_mut(), // No JIT memory needed for ARM
            0,
        )
    }

    fn replace_function_return_boolean(src: FuncPtrInternal, value: bool) -> PatchGuard {
        Self::replace_function_with_other_function(src, unsafe {
            FuncPtrInternal::new(
                NonNull::new(if value { return_true } else { return_false } as *mut ())
                    .expect("Failed to create FuncPtrInternal"), // Should never fail
            )
        })
    }
}

fn return_true() -> bool {
    true
}

fn return_false() -> bool {
    false
}
