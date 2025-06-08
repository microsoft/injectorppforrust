use crate::injector_core::common::*;

#[cfg(target_arch = "aarch64")]
use super::patch_arm64::PatchArm64;

#[cfg(target_arch = "x86_64")]
use super::patch_amd64::PatchAmd64;

use super::patch_trait::PatchTrait;

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
    pub(crate) fn will_execute_guard(self, target: FuncPtrInternal) -> PatchGuard {
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
    pub(crate) fn will_return_boolean_guard(self, value: bool) -> PatchGuard {
        #[cfg(target_arch = "aarch64")]
        {
            PatchArm64::replace_function_return_boolean(self.func_ptr, value)
        }

        #[cfg(target_arch = "x86_64")]
        {
            PatchAmd64::replace_function_return_boolean(self.func_ptr.as_ptr() as *mut u8, value)
        }
    }
}
