/// Converts a function to a `FuncPtr`.
///
/// This macro handles both generic and non-generic functions:
/// - For generic functions, provide the function name and type parameters separately: `func!(function_name, fn(Type1, Type2))`
/// - For non-generic functions, simply provide the function: `func!(function_name, fn())`
///
/// # Lifetime Safety
///
/// When the function being faked involves references, you **must** specify the exact lifetimes
/// in the type signature. Eliding or changing lifetimes can cause undefined behavior.
///
/// For example, if a function returns `&'static str`, you must write `&'static str` in the
/// type signature — not `&str` (which implies a lifetime linked to the input). Mismatched
/// lifetimes allow the fake to return dangling references. See GitHub issue #73 for details.
///
/// Use [`verify_func!`] after `func!` to add a compile-time check that catches some lifetime
/// mismatches (works for functions whose return type is independent of input lifetimes).
#[macro_export]
macro_rules! func {
    // Case 1: Generic function — provide function name and types separately
    ($f:ident :: <$($gen:ty),*>, $fn_type:ty) => {{
        let fn_val:$fn_type = $f::<$($gen),*>;
        let ptr = fn_val as *const ();
        let sig = std::any::type_name_of_val(&fn_val);
        let type_id = std::any::TypeId::of::<$fn_type>();

        unsafe { FuncPtr::new_with_type_id(ptr, sig, type_id) }
    }};

    // Case 2: Non-generic function
    ($f:expr, $fn_type:ty) => {{
        let fn_val:$fn_type = $f;
        let ptr = fn_val as *const ();
        let sig = std::any::type_name_of_val(&fn_val);
        let type_id = std::any::TypeId::of::<$fn_type>();

        unsafe { FuncPtr::new_with_type_id(ptr, sig, type_id) }
    }};

    // Simplified fn with return
    (func_info: fn ( $f:expr ) ( $($arg_ty:ty),* ) -> $ret:ty) => {{
        $crate::func!($f, fn($($arg_ty),*) -> $ret)
    }};

    (fn ( $f:expr ) ( $($arg_ty:ty),* ) -> $ret:ty) => {{
        $crate::func!($f, fn($($arg_ty),*) -> $ret)
    }};

    // Simplified fn with unit return
    (func_info: fn ( $f:expr ) ( $($arg_ty:ty),* )) => {{
        $crate::func!($f, fn($($arg_ty),*))
    }};

    (fn ( $f:expr ) ( $($arg_ty:ty),* )) => {{
        $crate::func!($f, fn($($arg_ty),*))
    }};

    // Simplified unsafe fn with return
    (func_info: unsafe fn ( $f:expr ) ( $($arg_ty:ty),* ) -> $ret:ty) => {{
        $crate::func!($f, unsafe fn($($arg_ty),*) -> $ret)
    }};

    (unsafe{} fn ( $f:expr ) ( $($arg_ty:ty),* ) -> $ret:ty) => {{
        $crate::func!($f, unsafe fn($($arg_ty),*) -> $ret)
    }};

    // Simplified unsafe fn with unit return
    (func_info: unsafe fn ( $f:expr ) ( $($arg_ty:ty),* )) => {{
        $crate::func!($f, unsafe fn($($arg_ty),*) -> ())
    }};

    (unsafe{} fn ( $f:expr ) ( $($arg_ty:ty),* )) => {{
        $crate::func!($f, unsafe fn($($arg_ty),*) -> ())
    }};

    // Simplified unsafe extern "C" fn with return
    (func_info: unsafe extern "C" fn ( $f:expr ) ( $($arg_ty:ty),* ) -> $ret:ty) => {{
        $crate::func!($f, unsafe extern "C" fn($($arg_ty),*) -> $ret)
    }};

    (unsafe{} extern "C" fn ( $f:expr ) ( $($arg_ty:ty),* ) -> $ret:ty) => {{
        $crate::func!($f, unsafe extern "C" fn($($arg_ty),*) -> $ret)
    }};

    // Simplified unsafe extern "C" fn with unit return
    (func_info: unsafe extern "C" fn ( $f:expr ) ( $($arg_ty:ty),* )) => {{
        $crate::func!($f, unsafe extern "C" fn($($arg_ty),*) -> ())
    }};

    (unsafe{} extern "C" fn ( $f:expr ) ( $($arg_ty:ty),* )) => {{
        $crate::func!($f, unsafe extern "C" fn($($arg_ty),*) -> ())
    }};

    // Simplified unsafe extern "system" fn with return
    (func_info: unsafe extern "system" fn ( $f:expr ) ( $($arg_ty:ty),* ) -> $ret:ty) => {{
        $crate::func!($f, unsafe extern "system" fn($($arg_ty),*) -> $ret)
    }};

    (unsafe{} extern "system" fn ( $f:expr ) ( $($arg_ty:ty),* ) -> $ret:ty) => {{
        $crate::func!($f, unsafe extern "system" fn($($arg_ty),*) -> $ret)
    }};

    // Simplified unsafe extern "system" fn with unit return
    (func_info: unsafe extern "system" fn ( $f:expr ) ( $($arg_ty:ty),* )) => {{
        $crate::func!($f, unsafe extern "system" fn($($arg_ty),*) -> ())
    }};

    (unsafe{} extern "system" fn ( $f:expr ) ( $($arg_ty:ty),* )) => {{
        $crate::func!($f, unsafe extern "system" fn($($arg_ty),*) -> ())
    }};
}

/// Converts a function to a `FuncPtr`.
///
/// This macro handles both generic and non-generic functions:
/// - For generic functions, provide the function name and type parameters separately: `func!(function_name::<Type1, Type2>)`
/// - For non-generic functions, simply provide the function: `func!(function_name)`
///
/// # Safety
///
/// This macro uses unsafe code internally and comes with the following requirements:
/// - The function pointer must remain valid for the entire duration it's used by injectorpp
/// - The function signature must match exactly what the injectorpp expects at runtime
/// - Mismatched function signatures will lead to undefined behavior or memory corruption
/// - Function pointers created with this macro should only be used with the appropriate injectorpp APIs
#[macro_export]
macro_rules! func_unchecked {
    // Case 1: Generic function — provide function name and types separately
    ($f:ident :: <$($gen:ty),*>) => {{
        let fn_val = $f::<$($gen),*>;
        let ptr = fn_val as *const ();

        FuncPtr::new(ptr, "")
    }};

    // Case 2: Non-generic function
    ($f:expr) => {{
        let fn_val = $f;
        let ptr = fn_val as *const ();

        FuncPtr::new(ptr, "")
    }};
}

/// Converts a closure to a `FuncPtr`.
///
/// This macro allows you to use Rust closures as mock implementations in injectorpp
/// by converting them to function pointers.
///
/// # Parameters
///
/// - `$closure`: The closure to convert
/// - `$fn_type`: The explicit function type signature that the closure conforms to
#[macro_export]
macro_rules! closure {
    ($closure:expr, $fn_type:ty) => {{
        let fn_val: $fn_type = $closure;
        let sig = std::any::type_name_of_val(&fn_val);
        let type_id = std::any::TypeId::of::<$fn_type>();

        unsafe { FuncPtr::new_with_type_id(fn_val as *const (), sig, type_id) }
    }};
}

/// Converts a closure to a `FuncPtr`.
///
/// This macro allows you to use Rust closures as mock implementations in injectorpp
/// by converting them to function pointers.
///
/// # Parameters
///
/// - `$closure`: The closure to convert
/// - `$fn_type`: The explicit function type signature that the closure conforms to
///
/// # Safety
///
/// This macro uses unsafe code internally and comes with significant safety requirements:
/// - The closure's signature must exactly match the provided function type
/// - The closure must not capture any references or variables with lifetimes shorter than the mock's usage
/// - The closure must remain valid for the entire duration it's used by injectorpp
/// - Mismatched function signatures will lead to undefined behavior or memory corruption
#[macro_export]
macro_rules! closure_unchecked {
    ($closure:expr, $fn_type:ty) => {{
        let fn_val: $fn_type = $closure;
        FuncPtr::new(fn_val as *const (), "")
    }};
}

#[doc(hidden)]
pub fn __assert_future_output<Fut, T>(_: &mut Fut)
where
    Fut: std::future::Future<Output = T>,
{
}

/// Helper to extract TypeId from a value's type. Used internally by macros.
#[doc(hidden)]
pub fn __type_id_of_val<T: 'static>(_: &T) -> std::any::TypeId {
    std::any::TypeId::of::<T>()
}

/// Ensure the async function can be correctly used in injectorpp.
#[macro_export]
macro_rules! async_func {
    ($expr:expr, $ty:ty) => {{
        let mut __fut = $expr;

        let _ = __assert_future_output::<_, $ty>(&mut __fut);

        let sig = std::any::type_name::<fn() -> std::task::Poll<$ty>>();
        (std::pin::pin!(__fut), sig)
    }};
}

/// Ensure the async function can be correctly used in injectorpp.
///
/// # Safety
///
/// This macro skips the signature check and assumes the caller knows what they are doing.
#[macro_export]
macro_rules! async_func_unchecked {
    ($expr:expr) => {
        std::pin::pin!($expr)
    };
}

/// Config a return value for faking an async function.
#[macro_export]
macro_rules! async_return {
    ($val:expr, $ty:ty) => {{
        fn generated_poll_fn() -> std::task::Poll<$ty> {
            std::task::Poll::Ready($val)
        }

        $crate::func!(generated_poll_fn, fn() -> std::task::Poll<$ty>)
    }};
}

/// Config a return value for faking an async function.
///
/// # Safety
///
/// This macro skips the signature check and assumes the caller knows what they are doing.
#[macro_export]
macro_rules! async_return_unchecked {
    ($val:expr, $ty:ty) => {{
        fn generated_poll_fn() -> std::task::Poll<$ty> {
            std::task::Poll::Ready($val)
        }

        $crate::func_unchecked!(generated_poll_fn)
    }};
}

/// Creates a mock function implementation with configurable behavior and verification.
///
/// This macro generates a function that can be used to replace real functions during testing.
/// It supports configuring return values, parameter validation, side effects through
/// reference parameters, and verification of call counts.
///
/// # Parameters
///
/// - `func_type`: Required. The function signature to mock (e.g., `fn(x: i32) -> bool`).
/// - `when`: Optional. A condition on the function parameters that must be true for the mock to execute.
/// - `assign`: Optional. Code block to execute for modifying reference parameters.
/// - `returns`: Required for non-unit functions. The value to return from the mock.
/// - `times`: Optional. Verifies the function is called exactly this many times.
///
/// # Safety
///
/// This macro uses unsafe code internally and comes with significant safety requirements:
/// - The function signature must exactly match the signature of the function being mocked
/// - The mock must handle all possible input parameters correctly
/// - Memory referenced by parameters must remain valid for the duration of the function call
/// - Type mismatches between the mocked function and its implementation will cause undefined behavior
/// - Mock functions created with this macro must only be used with the `will_execute` method
#[macro_export]
macro_rules! fake {
    // === NON-UNIT RETURNING FUNCTIONS (return type not "()") ===

    // With when, assign, returns, and times.
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) -> $ret {
             if $cond {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 { $($assign)* }
                 $ret_val
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when, assign, and returns (no times).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) -> $ret {
             if $cond {
                 { $($assign)* }
                 $ret_val
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and returns, times, but no assign.
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) -> $ret {
             if $cond {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 $ret_val
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and returns (no times, no assign).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        returns: $ret_val:expr
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) -> $ret {
             if $cond {
                 $ret_val
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        returns: $ret_val:expr
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         unsafe extern "C" fn fake($($arg_name: $arg_ty),*) -> $ret {
             if $cond {
                 $ret_val
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: unsafe extern "C" fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign, returns and times
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) -> $ret {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 { $($assign)* }
                 $ret_val
             } else {
                unreachable!()
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign and returns
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) -> $ret {
             if true {
                { $($assign)* }
                 $ret_val
             } else {
                 unreachable!()
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With times and returns
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) -> $ret {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 $ret_val
             } else {
                 unreachable!()
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With returns only.
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        returns: $ret_val:expr
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) -> $ret {
             if true {
                 $ret_val
             } else {
                 unreachable!()
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        returns: $ret_val:expr
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         unsafe extern "C" fn fake($($arg_name: $arg_ty),*) -> $ret {
             if true {
                 $ret_val
             } else {
                 unreachable!()
             }
         }
         let f: unsafe extern "C" fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};

    // === UNIT RETURNING FUNCTIONS (-> ()) ===

    // With when, assign, and times.
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        assign: { $($assign:tt)* },
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) {
             if $cond {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 { $($assign)* }
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and times (no assign).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) {
             if $cond {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and assign (no times).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        assign: { $($assign:tt)* }
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) {
             if $cond {
                 { $($assign)* }
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign only
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* }
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) {
             if true {
                 { $($assign)* }
             } else {
                unreachable!()
             }
         }
         let f: fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign and times
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* },
        times: $expected:expr
    ) => {{

        use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 { $($assign)* }
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With times only (when defaults to true, no assign).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
             } else {
                 unreachable!()
             }
         }
         let f: fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With neither (no when, no times, no assign, no returns).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> ()
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) {
             if true { } else { unreachable!() }
         }
         let f: fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};

    // === NORMAL UNSAFE NON-UNIT RETURNING FUNCTIONS ===
    // With returns only for unsafe fn
    (
        func_type: unsafe fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        returns: $ret_val:expr
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With returns and times for unsafe fn
    (
        func_type: unsafe fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign and returns for unsafe fn
    (
        func_type: unsafe fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                { $($assign)* }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign, returns, and times for unsafe fn
    (
        func_type: unsafe fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                { $($assign)* }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // === NORMAL UNSAFE UNIT RETURNING FUNCTIONS ===
    // With times for unsafe fn
    (
        func_type: unsafe fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe fn fake($($arg_name: $arg_ty),*) {
            if true {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
            } else {
                unreachable!()
            }
        }
        let f: unsafe fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign only
    (
        func_type: unsafe fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* }
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe fn fake($($arg_name: $arg_ty),*) {
            if true {
                { $($assign)* }
            } else {
                unreachable!()
            }
        }
        let f: unsafe fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign and times
    (
        func_type: unsafe fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* },
        times: $expected:expr
    ) => {{

        use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         unsafe fn fake($($arg_name: $arg_ty),*) {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 { $($assign)* }
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: unsafe fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // Without times for unsafe fn
    (
        func_type: unsafe fn($($arg_name:ident: $arg_ty:ty),*) -> ()
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe fn fake($($arg_name: $arg_ty),*) {
            if true { } else { unreachable!() }
        }
        let f: unsafe fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};

    // === EXTERN "C" NON-UNIT RETURNING FUNCTIONS ===
    // With when, assign, returns, and times.
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if $cond {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                { $($assign)* }
                $ret_val
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when, assign, and returns
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if $cond {
                { $($assign)* }
                $ret_val
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and returns, times
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if $cond {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                $ret_val
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign, returns, and times
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                { $($assign)* }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign and returns
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                { $($assign)* }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With returns and times
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // === EXTERN "C" UNIT RETURNING FUNCTIONS ===
    // With when, assign, and times
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        assign: { $($assign:tt)* },
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) {
            if $cond {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                { $($assign)* }
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and times (no assign).
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) {
            if $cond {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and assign (no times).
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        assign: { $($assign:tt)* }
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) {
            if $cond {
                { $($assign)* }
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign only
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* }
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe extern "C" fn fake($($arg_name: $arg_ty),*) {
            if true {
                { $($assign)* }
            } else {
                unreachable!()
            }
        }
        let f: unsafe extern "C" fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign and times
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* },
        times: $expected:expr
    ) => {{

        use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         unsafe extern "C" fn fake($($arg_name: $arg_ty),*) {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 { $($assign)* }
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: unsafe extern "C" fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With times only (when defaults to true, no assign).
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         unsafe extern "C" fn fake($($arg_name: $arg_ty),*) {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
             } else {
                 unreachable!()
             }
         }
         let f: unsafe extern "C" fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With neither (no when, no times, no assign, no returns).
    (
        func_type: unsafe extern "C" fn($($arg_name:ident: $arg_ty:ty),*) -> ()
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         unsafe extern "C" fn fake($($arg_name: $arg_ty),*) {
             if true { } else { unreachable!() }
         }
         let f: unsafe extern "C" fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // === EXTERN "system" NON-UNIT RETURNING FUNCTIONS ===
    // With when, assign, returns, and times.
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if $cond {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                { $($assign)* }
                $ret_val
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when, assign, and returns
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if $cond {
                { $($assign)* }
                $ret_val
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and returns, times
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        when: $cond:expr,
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if $cond {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                $ret_val
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign, returns, and times
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                { $($assign)* }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign and returns
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        assign: { $($assign:tt)* },
        returns: $ret_val:expr
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                { $($assign)* }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With returns and times
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        returns: $ret_val:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) -> $ret {
            if true {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                $ret_val
            } else {
                unreachable!()
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) -> $ret = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> $ret:ty,
        returns: $ret_val:expr
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         unsafe extern "system" fn fake($($arg_name: $arg_ty),*) -> $ret {
             if true {
                 $ret_val
             } else {
                 unreachable!()
             }
         }
         let f: unsafe extern "system" fn($($arg_ty),*) -> $ret = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // === EXTERN "system" UNIT RETURNING FUNCTIONS ===
    // With when, assign, and times
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        assign: { $($assign:tt)* },
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) {
            if $cond {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
                { $($assign)* }
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and times (no assign).
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        times: $expected:expr
    ) => {{
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) {
            if $cond {
                let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                if prev >= $expected {
                    panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                }
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With when and assign (no times).
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        assign: { $($assign:tt)* }
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) {
            if $cond {
                { $($assign)* }
            } else {
                panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign only
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* }
    ) => {{
        let verifier = CallCountVerifier::Dummy;
        unsafe extern "system" fn fake($($arg_name: $arg_ty),*) {
            if true {
                { $($assign)* }
            } else {
                unreachable!()
            }
        }
        let f: unsafe extern "system" fn($($arg_ty),*) = fake;
        let raw_ptr = f as *const ();
        (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With assign and times
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* },
        times: $expected:expr
    ) => {{

        use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         unsafe extern "system" fn fake($($arg_name: $arg_ty),*) {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
                 { $($assign)* }
             } else {
                 panic!("Fake function defined at {}:{}:{} called with unexpected arguments", file!(), line!(), column!());
             }
         }
         let f: unsafe extern "system" fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With times only (when defaults to true, no assign).
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         unsafe extern "system" fn fake($($arg_name: $arg_ty),*) {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function defined at {}:{}:{} called more times than expected", file!(), line!(), column!());
                 }
             } else {
                 unreachable!()
             }
         }
         let f: unsafe extern "system" fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
    // With neither (no when, no times, no assign, no returns).
    (
        func_type: unsafe extern "system" fn($($arg_name:ident: $arg_ty:ty),*) -> ()
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         unsafe extern "system" fn fake($($arg_name: $arg_ty),*) {
             if true { } else { unreachable!() }
         }
         let f: unsafe extern "system" fn($($arg_ty),*) = fake;
         let raw_ptr = f as *const ();
         (unsafe { FuncPtr::new_with_type_id(raw_ptr, std::any::type_name_of_val(&f), __type_id_of_val(&f)) }, verifier)
    }};
}

/// Compile-time check that the specified function signature matches the function's actual type.
///
/// This macro catches lifetime mismatches that `func!` cannot detect on its own due to Rust's
/// implicit function pointer subtyping. It uses type invariance (`&mut T`) to prevent coercion
/// and will produce a compile error if the lifetimes don't match.
///
/// # When to use
///
/// Use this macro when faking functions that return references, especially `&'static` references.
/// It prevents the common mistake of eliding `'static` (e.g., writing `&str` instead of
/// `&'static str`), which can cause undefined behavior (see issue #73).
///
/// # Limitations
///
/// This macro only works for functions whose return type is **independent** of input lifetimes.
/// It cannot be used with functions like `fn(&str) -> &str` where the output borrows from
/// the input — use `func!` directly for those.
///
/// # Example
///
/// ```rust,compile_fail
/// fn foo(_s: &str) -> &'static str { "abc" }
///
/// // This correctly fails to compile — the return type should be &'static str, not &str
/// injectorpp::verify_func!(fn (foo)(&str) -> &str);
/// ```
///
/// ```rust
/// fn foo(_s: &str) -> &'static str { "abc" }
///
/// // This compiles — the return type correctly matches
/// injectorpp::verify_func!(fn (foo)(&str) -> &'static str);
/// ```
#[macro_export]
macro_rules! verify_func {
    (fn ($func:expr)($($param:ty),*) -> $ret:ty) => {{
        #[allow(non_snake_case)]
        fn __injectorpp_infer_ret<__InjectorppRet>(
            _f: fn($($param),*) -> __InjectorppRet,
        ) -> fn($($param),*) -> __InjectorppRet {
            _f
        }
        #[allow(non_snake_case)]
        fn __injectorpp_must_match<__InjectorppT>(
            _a: &mut __InjectorppT,
            _b: &mut __InjectorppT,
        ) {
        }
        let mut __injectorpp_natural = __injectorpp_infer_ret($func);
        let mut __injectorpp_user: fn($($param),*) -> $ret = $func;
        __injectorpp_must_match(&mut __injectorpp_natural, &mut __injectorpp_user);
    }};
}
