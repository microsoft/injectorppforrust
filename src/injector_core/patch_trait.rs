use crate::injector_core::common::*;

pub(crate) trait PatchTrait {
    fn replace_function_with_other_function(
        src: FuncPtrInternal,
        target: FuncPtrInternal,
    ) -> PatchGuard;

    fn replace_function_return_boolean(src: FuncPtrInternal, value: bool) -> PatchGuard;
}
