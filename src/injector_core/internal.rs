use crate::injector_core::common::*;

#[cfg(target_arch = "aarch64")]
use super::patch_arm64::PatchArm64;

#[cfg(target_arch = "x86_64")]
use super::patch_amd64::PatchAmd64;

use super::patch_trait::PatchTrait;

/// An internal builder for patching a function. Not exposed publicly.
pub struct WhenCalled {
    func_ptr: *mut u8,
}

impl WhenCalled {
    pub fn new(func: *const ()) -> Self {
        Self {
            func_ptr: func as *mut u8,
        }
    }

    /// Patches the target function so that it branches to a JIT block that uses an absolute jump
    /// to call the target function.
    pub fn will_execute_guard(self, target: *const ()) -> PatchGuard {
        #[cfg(target_arch = "aarch64")]
        {
            PatchArm64::replace_function_with_other_function(self.func_ptr, target)
        }

        #[cfg(target_arch = "x86_64")]
        {
            PatchAmd64::replace_function_with_other_function(self.func_ptr, target)
        }
    }

    /// Patches the target function so that it branches to a JIT block that returns the specified boolean.
    pub fn will_return_boolean_guard(self, value: bool) -> PatchGuard {
        #[cfg(target_arch = "aarch64")]
        {
            PatchArm64::replace_function_return_boolean(self.func_ptr, value)
        }

        #[cfg(target_arch = "x86_64")]
        {
            PatchAmd64::replace_function_return_boolean(self.func_ptr, value)
        }
    }
}
