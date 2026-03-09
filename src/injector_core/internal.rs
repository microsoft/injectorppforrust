use crate::injector_core::common::*;

#[cfg(target_arch = "aarch64")]
use super::patch_arm64::PatchArm64;

#[cfg(target_arch = "arm")]
use super::patch_arm::PatchArm;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
use super::patch_trait::PatchTrait;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use super::thread_local_registry;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use super::thread_local_registry::ThreadRegistration;

/// An internal builder for patching a function. Not exposed publicly.
pub(crate) struct WhenCalled {
    func_ptr: FuncPtrInternal,
}

impl WhenCalled {
    pub(crate) fn new(func: FuncPtrInternal) -> Self {
        Self { func_ptr: func }
    }

    /// Patches the target function so that it branches to a JIT block that uses an absolute jump
    /// to call the target function.
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub(crate) fn will_execute_guard(self, target: FuncPtrInternal) -> PatchGuard {
        #[cfg(target_arch = "aarch64")]
        {
            PatchArm64::replace_function_with_other_function(self.func_ptr, target)
        }

        #[cfg(target_arch = "arm")]
        {
            PatchArm::replace_function_with_other_function(self.func_ptr, target)
        }
    }

    /// Patches the target function using thread-local dispatch (x86_64 only).
    /// The original function is patched to a dispatcher that routes calls
    /// to per-thread replacement functions.
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    pub(crate) fn will_execute_thread_local(
        self,
        target: FuncPtrInternal,
    ) -> ThreadRegistration {
        let replacement_addr = target.as_ptr() as usize;
        thread_local_registry::register_replacement(&self.func_ptr, replacement_addr, None)
    }

    /// Patches the target function to return a boolean using thread-local dispatch.
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    pub(crate) fn will_return_boolean_thread_local(self, value: bool) -> ThreadRegistration {
        // Generate a small JIT block that returns the boolean value
        #[cfg(target_arch = "x86_64")]
        let (jit_size, asm_code_vec) = {
            let code: [u8; 8] = [
                0x48, 0xC7, 0xC0, // mov rax, imm32
                value as u8, 0x00, 0x00, 0x00, // imm32
                0xC3, // ret
            ];
            (8usize, code.to_vec())
        };

        #[cfg(target_arch = "aarch64")]
        let (jit_size, asm_code_vec) = {
            use super::arm64_codegenerator::*;
            use super::utils::u8_to_bits;

            let mut value_bits = [false; 16];
            value_bits[0] = value;
            let movz = emit_movz(value_bits, true, u8_to_bits::<2>(0), u8_to_bits::<5>(0));
            let ret = emit_ret_x30();

            let mut code = Vec::with_capacity(8);
            code.extend_from_slice(&bool_array_to_u32(movz).to_le_bytes());
            code.extend_from_slice(&bool_array_to_u32(ret).to_le_bytes());
            (8usize, code)
        };

        let jit_memory = allocate_jit_memory(&self.func_ptr, jit_size);

        unsafe {
            inject_asm_code(&asm_code_vec, jit_memory);
        }

        let replacement_addr = jit_memory as usize;
        thread_local_registry::register_replacement(
            &self.func_ptr,
            replacement_addr,
            Some((jit_memory, jit_size)),
        )
    }

    /// Patches the target function so that it branches to a JIT block that returns the specified boolean.
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub(crate) fn will_return_boolean_guard(self, value: bool) -> PatchGuard {
        #[cfg(target_arch = "aarch64")]
        {
            PatchArm64::replace_function_return_boolean(self.func_ptr, value)
        }

        #[cfg(target_arch = "arm")]
        {
            PatchArm::replace_function_return_boolean(self.func_ptr, value)
        }
    }
}
