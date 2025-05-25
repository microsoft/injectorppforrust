use crate::injector_core::common::*;

pub trait PatchTrait {
    fn replace_function_with_other_function(src: *mut u8, target: *const ()) -> PatchGuard;
    fn replace_function_return_boolean(src: *mut u8, value: bool) -> PatchGuard;
}
