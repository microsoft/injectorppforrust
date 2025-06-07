use crate::injector_core::common::*;
use crate::injector_core::internal::*;

use std::future::Future;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::*;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::task::Context;
use std::task::Poll;

/// A `Mutex` that never stays poisoned: on panic it just recovers the guard.
///
/// This is a trade-off between user experience and potential data corrupt issue.
/// When panic happens in the multi thread scenario, the std Mutex will cause poison error.
/// This will fail other unrelated test cases. The test failure accuracy is
/// more important to users so ignore the poison error.
pub struct NoPoisonMutex<T> {
    inner: Mutex<T>,
}

impl<T> NoPoisonMutex<T> {
    /// Create a new mutex.
    pub const fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
        }
    }

    /// Lock, recovering if the mutex was poisoned.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // Swallow the poison and give the guard anyway
                poisoned.into_inner()
            }
        }
    }
}

static LOCK_FUNCTION: NoPoisonMutex<()> = NoPoisonMutex::new(());

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
macro_rules! func {
    // Case 1: Generic function — provide function name and types separately
    ($f:ident :: <$($gen:ty),*>) => {{
        let ptr = $f::<$($gen),*>;
        unsafe { FuncPtr::new(ptr as *const ()) }
    }};

    // Case 2: Non-generic function
    ($f:expr) => {
        unsafe { FuncPtr::new($f as *const ()) }
    };
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
macro_rules! closure {
    ($closure:expr, $fn_type:ty) => {{
        let fn_ptr: $fn_type = $closure;
        unsafe { FuncPtr::new(fn_ptr as *const ()) }
    }};
}

// Ensure the async function can be correctly used in injectorpp.
#[macro_export]
macro_rules! async_func {
    ($expr:expr) => {
        std::pin::pin!($expr)
    };
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
                     panic!("Fake function called more times than expected");
                 }
                 { $($assign)* }
                 $ret_val
             } else {
                 panic!("Fake function called with unexpected arguments");
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> $ret) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
                 panic!("Fake function called with unexpected arguments");
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> $ret) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
                     panic!("Fake function called more times than expected");
                 }
                 $ret_val
             } else {
                 panic!("Fake function called with unexpected arguments");
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> $ret) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
                 panic!("Fake function called with unexpected arguments");
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> $ret) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
                     panic!("Fake function called more times than expected");
                 }
                 { $($assign)* }
                 $ret_val
             } else {
                unreachable!()
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> $ret) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
         let raw_ptr = (fake as fn($($arg_ty),*) -> $ret) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
                     panic!("Fake function called more times than expected");
                 }
                 $ret_val
             } else {
                 unreachable!()
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> $ret) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
         let raw_ptr = (fake as fn($($arg_ty),*) -> $ret) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
         fn fake($($arg_name: $arg_ty),*) -> () {
             if $cond {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function called more times than expected");
                 }
                 { $($assign)* }
             } else {
                 panic!("Fake function called with unexpected arguments");
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> ()) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
         fn fake($($arg_name: $arg_ty),*) -> () {
             if $cond {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function called more times than expected");
                 }
                 ()
             } else {
                 panic!("Fake function called with unexpected arguments");
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> ()) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
    }};
    // With when and assign (no times).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        when: $cond:expr,
        assign: { $($assign:tt)* }
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) -> () {
             if $cond {
                 { $($assign)* }
             } else {
                 panic!("Fake function called with unexpected arguments");
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> ()) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
    }};
    // With assign only
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        assign: { $($assign:tt)* }
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) -> () {
             if true {
                 { $($assign)* }
             } else {
                unreachable!()
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> ()) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
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
         fn fake($($arg_name: $arg_ty),*) -> () {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function called more times than expected");
                 }
                 { $($assign)* }
                 ()
             } else {
                 panic!("Fake function called with unexpected arguments");
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> ()) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
    }};
    // With times only (when defaults to true, no assign).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> (),
        times: $expected:expr
    ) => {{
         use std::sync::atomic::{AtomicUsize, Ordering};
         static FAKE_COUNTER: AtomicUsize = AtomicUsize::new(0);
         let verifier = CallCountVerifier::WithCount { counter: &FAKE_COUNTER, expected: $expected };
         fn fake($($arg_name: $arg_ty),*) -> () {
             if true {
                 let prev = FAKE_COUNTER.fetch_add(1, Ordering::SeqCst);
                 if prev >= $expected {
                     panic!("Fake function called more times than expected");
                 }
                 ()
             } else {
                 unreachable!()
             }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> ()) as *const ();
         (unsafe { FuncPtr::new(raw_ptr) }, verifier)
    }};
    // With neither (no when, no times, no assign, no returns).
    (
        func_type: fn($($arg_name:ident: $arg_ty:ty),*) -> ()
    ) => {{
         let verifier = CallCountVerifier::Dummy;
         fn fake($($arg_name: $arg_ty),*) -> () {
             if true { () } else { unreachable!() }
         }
         let raw_ptr = (fake as fn($($arg_ty),*) -> ()) as *const ();
         (raw_ptr, verifier)
    }};
}

// Define a verifier guard that checks the counter on Drop.
/// A verifier type that holds a reference to an atomic counter and the expected call count.
pub enum CallCountVerifier {
    /// A real verifier that checks if the fake function was called the expected number of times.
    WithCount {
        counter: &'static AtomicUsize,
        expected: usize,
    },

    /// A dummy verifier that performs no check.
    Dummy,
}

impl Drop for CallCountVerifier {
    fn drop(&mut self) {
        if let CallCountVerifier::WithCount { counter, expected } = self {
            let call_times = counter.load(Ordering::SeqCst);
            if call_times != *expected {
                panic!(
                    "Fake function was expected to be called {} time(s), but it is actually called {} time(s)",
                    expected, call_times
                );
            }
        }

        // Dummy variant does nothing on drop.
    }
}

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
pub struct FuncPtr(NonNull<()>);

impl FuncPtr {
    /// Creates a new `FuncPtr` from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a function.
    pub unsafe fn new(ptr: *const ()) -> Self {
        // While these basic checks are performed, it is not a substitute for
        // proper function pointer validation. The caller must ensure that the
        // pointer is indeed a valid function pointer.
        let p = ptr as *mut ();
        let nn = NonNull::new(p).expect("Pointer must not be null");

        const MIN_FUNC_PTR_ALIGN: usize = std::mem::size_of::<usize>();
        assert!(
            (nn.as_ptr() as usize) % MIN_FUNC_PTR_ALIGN == 0,
            "Pointer has insufficient alignment for function pointer"
        );

        FuncPtr(nn)
    }

    /// Returns the raw pointer to the function.
    fn as_ptr(&self) -> *const () {
        self.0.as_ptr()
    }
}

/// A high-level type that holds patch guards so that when it goes out of scope,
/// the original function code is automatically restored.
///
/// # Thread Safety
///
/// InjectorPP ensures thread safety by holding a global mutex for the entire lifetime
/// of the patch. However, users must ensure that no other thread executes the patched
/// function after the InjectorPP instance is dropped. If multiple threads may execute
/// the patched function concurrently, ensure that InjectorPP instances remain alive
/// until all threads have completed execution of the patched function.
pub struct InjectorPP {
    guards: Vec<PatchGuard>,
    verifiers: Vec<CallCountVerifier>,
    _lock: MutexGuard<'static, ()>,
}

impl InjectorPP {
    /// Creates a new `InjectorPP` instance.
    ///
    /// `InjectorPP` allows faking Rust functions at runtime without modifying the original code.
    /// It ensures thread safety by holding a global mutex for the entire lifetime of the patch.
    ///
    /// # Example
    ///
    /// ```rust
    /// use injectorpp::interface::injector::InjectorPP;
    ///
    /// let injector = InjectorPP::new();
    /// ```
    pub fn new() -> Self {
        let lock = LOCK_FUNCTION.lock();

        Self {
            guards: Vec::new(),
            verifiers: Vec::new(),
            _lock: lock,
        }
    }

    /// Begins faking a function.
    ///
    /// Accepts a FuncPtr to the function you want to fake. Use the `func!` macro to obtain this pointer.
    ///
    /// # Parameters
    ///
    /// - `func`: A FuncPtr holds the pointer to the target function obtained via `func!` macro.
    ///
    /// # Returns
    ///
    /// A builder (`WhenCalledBuilder`) to further specify the fake behavior.
    ///
    /// # Example
    ///
    /// ```rust
    /// use injectorpp::interface::injector::*;
    /// use std::path::Path;
    ///
    /// fn fake_exists(_path: &Path) -> bool {
    ///     true
    /// }
    ///
    /// let mut injector = InjectorPP::new();
    /// injector
    ///     .when_called(injectorpp::func!(Path::exists))
    ///     .will_execute_raw(injectorpp::func!(fake_exists));
    ///
    /// assert!(Path::new("/non/existent/path").exists());
    /// ```
    pub fn when_called(&mut self, func: FuncPtr) -> WhenCalledBuilder<'_> {
        let when = WhenCalled::new(func.as_ptr());
        WhenCalledBuilder { lib: self, when }
    }

    /// Begins faking an asynchronous function.
    ///
    /// Accepts a pinned mutable reference to the async function future. Use the `async_func!` macro to obtain this reference.
    ///
    /// # Parameters
    ///
    /// - `_`: A pinned mutable reference to the async function future.
    ///
    /// # Returns
    ///
    /// A builder (`WhenCalledBuilderAsync`) to further specify the async fake behavior.
    ///
    /// # Example
    ///
    /// ```rust
    /// use injectorpp::interface::injector::*;
    ///
    /// async fn async_add_one(x: u32) -> u32 {
    ///     x + 1
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut injector = InjectorPP::new();
    ///     injector
    ///         .when_called_async(injectorpp::async_func!(async_add_one(u32::default())))
    ///         .will_return_async(injectorpp::async_return!(123, u32));
    ///
    ///     let result = async_add_one(5).await;
    ///     assert_eq!(result, 123); // The patched value
    /// }
    /// ```
    pub fn when_called_async<F, T>(&mut self, _: Pin<&mut F>) -> WhenCalledBuilderAsync<'_>
    where
        F: Future<Output = T>,
    {
        let poll_fn: fn(Pin<&mut F>, &mut Context<'_>) -> Poll<T> = <F as Future>::poll;
        let when = WhenCalled::new(func!(poll_fn).as_ptr());
        WhenCalledBuilderAsync { lib: self, when }
    }
}

impl Default for InjectorPP {
    fn default() -> Self {
        Self::new()
    }
}

/// A builder that lets you chain patching operations.
pub struct WhenCalledBuilder<'a> {
    lib: &'a mut InjectorPP,
    when: WhenCalled,
}

impl WhenCalledBuilder<'_> {
    /// Fake the target function to branch to the provided function.
    ///
    /// Allows full customization of the faked function behavior by providing your own function or closure.
    ///
    /// # Parameters
    ///
    /// - `target`: A FuncPtr holds the pointer to the replacement function or closure. Using injectorpp::func! or injectorpp::closure! macros is recommended to obtain this pointer.
    ///
    /// # Example
    ///
    /// Using closure:
    /// ```rust
    /// use injectorpp::interface::injector::*;
    /// use std::path::Path;
    ///
    /// let fake_closure = |_: &Path| -> bool {
    ///    true
    /// };
    ///
    /// let mut injector = InjectorPP::new();
    /// injector
    ///     .when_called(injectorpp::func!(Path::exists))
    ///     .will_execute_raw(injectorpp::closure!(fake_closure, fn(&Path) -> bool));
    ///
    /// assert!(Path::new("/nonexistent").exists());
    /// ```
    ///
    /// Using custom function:
    /// ```rust
    /// use injectorpp::interface::injector::*;
    /// use std::path::Path;
    ///
    /// fn fake_exists(_path: &Path) -> bool {
    ///     true
    /// }
    ///
    /// let mut injector = InjectorPP::new();
    /// injector
    ///     .when_called(injectorpp::func!(Path::exists))
    ///     .will_execute_raw(injectorpp::func!(fake_exists));
    ///
    /// assert!(Path::new("/nonexistent").exists());
    /// ```
    pub fn will_execute_raw(self, target: FuncPtr) {
        let guard = self.when.will_execute_guard(target.as_ptr());
        self.lib.guards.push(guard);
    }

    /// Fake the target function using a fake function generated by the `fake!` macro.
    ///
    /// Suitable for complex scenarios where you specify conditions, assignments, return values, and expected call counts.
    ///
    /// # Example
    ///
    /// ```rust
    /// use injectorpp::interface::injector::*;
    ///
    /// fn original_func(a: &mut i32) -> bool {
    ///     *a = 1;
    ///     false
    /// }
    ///
    /// let mut injector = InjectorPP::new();
    /// injector
    ///     .when_called(injectorpp::func!(original_func))
    ///     .will_execute(injectorpp::fake!(
    ///         func_type: fn(a: &mut i32) -> bool,
    ///         assign: { *a = 6 },
    ///         returns: true,
    ///         times: 1
    ///     ));
    ///
    /// let mut value = 0;
    /// let result = original_func(&mut value);
    ///
    /// assert_eq!(value, 6);
    /// assert_eq!(result, true);
    /// ```
    /// Below are supported options:
    ///
    /// `func_type``: // Required. The signature of the function to fake.
    /// `when``: // Optional. A condition check for the parameters of the function to fake.
    /// `assign``: // Optional. Use to set values to reference variables of the function to fake.
    /// `returns``: // Required for the function has return. Specify what the return value should be.
    /// `times``: // Optional. How many times the function should be called. If the value is not satisfied at the end of the test, the test will fail.
    pub fn will_execute(self, fake_pair: (FuncPtr, CallCountVerifier)) {
        let (fake_func, verifier) = fake_pair;
        self.lib.verifiers.push(verifier);
        self.will_execute_raw(fake_func);
    }

    /// Fake the target function to always return a fixed boolean value.
    ///
    /// This method is convenient for functions that return boolean values.
    ///
    /// # Example
    ///
    /// ```rust
    /// use injectorpp::interface::injector::*;
    /// use std::path::Path;
    ///
    /// let mut injector = InjectorPP::new();
    /// injector
    ///     .when_called(injectorpp::func!(Path::exists))
    ///     .will_return_boolean(true);
    ///
    /// assert!(Path::new("/nonexistent").exists());
    /// ```
    pub fn will_return_boolean(self, value: bool) {
        let guard = self.when.will_return_boolean_guard(value);
        self.lib.guards.push(guard);
    }
}

pub struct WhenCalledBuilderAsync<'a> {
    lib: &'a mut InjectorPP,
    when: WhenCalled,
}

#[macro_export]
macro_rules! async_return {
    ($val:expr, $ty:ty) => {{
        fn generated_poll_fn() -> std::task::Poll<$ty> {
            std::task::Poll::Ready($val)
        }

        $crate::func!(generated_poll_fn)
    }};
}

impl WhenCalledBuilderAsync<'_> {
    /// Fake the target async function to return a specified async value.
    ///
    /// This method allows you to fake async functions by specifying the return value directly.
    ///
    /// # Example
    ///
    /// ```rust
    /// use injectorpp::interface::injector::*;
    ///
    /// async fn async_func_bool(x: bool) -> bool {
    ///     x
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut injector = InjectorPP::new();
    ///     injector
    ///         .when_called_async(injectorpp::async_func!(async_func_bool(true)))
    ///         .will_return_async(injectorpp::async_return!(false, bool));
    ///
    ///     let result = async_func_bool(true).await;
    ///     assert_eq!(result, false);
    /// }
    /// ```
    pub fn will_return_async(self, target: FuncPtr) {
        let guard = self.when.will_execute_guard(target.as_ptr());
        self.lib.guards.push(guard);
    }
}
