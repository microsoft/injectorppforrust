use crate::injector_core::common::FuncPtrInternal;
use std::any::TypeId;
use std::ptr::NonNull;

/// A safe wrapper around a raw function pointer.
///
/// `FuncPtr` encapsulates a non-null function pointer and provides safe
/// creation and access methods. It's used throughout injectorpp
/// to represent both original functions to be mocked and their replacement
/// implementations.
///
/// # Safety
///
/// The caller must ensure that the pointer is valid and points to a function.
pub struct FuncPtr {
    /// The internal representation of the function pointer.
    ///
    /// This is a wrapper around a non-null pointer to ensure safety.
    pub(super) func_ptr_internal: FuncPtrInternal,
    pub(super) signature: &'static str,
    pub(super) type_id: Option<TypeId>,
}

impl FuncPtr {
    /// Creates a new `FuncPtr` from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a function.
    pub unsafe fn new(ptr: *const (), signature: &'static str) -> Self {
        // While these basic checks are performed, it is not a substitute for
        // proper function pointer validation. The caller must ensure that the
        // pointer is indeed a valid function pointer.
        let p = ptr as *mut ();
        let nn = NonNull::new(p).expect("Pointer must not be null");

        Self {
            func_ptr_internal: FuncPtrInternal::new(nn),
            signature,
            type_id: None,
        }
    }

    /// Creates a new `FuncPtr` from a raw pointer with type identity information.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a function.
    pub unsafe fn new_with_type_id(
        ptr: *const (),
        signature: &'static str,
        type_id: TypeId,
    ) -> Self {
        let p = ptr as *mut ();
        let nn = NonNull::new(p).expect("Pointer must not be null");

        Self {
            func_ptr_internal: FuncPtrInternal::new(nn),
            signature,
            type_id: Some(type_id),
        }
    }
}
