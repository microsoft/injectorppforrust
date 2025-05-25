use crate::injector_core::common::*;
use crate::injector_core::internal::*;

use std::future::Future;
use std::pin::Pin;
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

// Convert the function to a raw pointer that can be used by injector.
#[macro_export]
macro_rules! func {
    // Case 1: Generic function — provide function name and types separately
    ($f:ident :: <$($gen:ty),*>) => {{
        let ptr = $f::<$($gen),*>;
        ptr as *const ()
    }};

    // Case 2: Non-generic function
    ($f:expr) => {
        $f as *const ()
    };
}

// Convert the closure to a raw pointer that can be used by injector.
#[macro_export]
macro_rules! closure {
    ($closure:expr, $fn_type:ty) => {{
        let fn_ptr: $fn_type = $closure;
        fn_ptr as *const ()
    }};
}

// Ensure the async function can be correctly used in injectorpp.
#[macro_export]
macro_rules! async_func {
    ($expr:expr) => {
        std::pin::pin!($expr)
    };
}

// Macro to generate a fake function with a clear, named syntax.
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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
         (raw_ptr, verifier)
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

/// A high-level type that holds patch guards so that when it goes out of scope,
/// the original function code is automatically restored.
pub struct InjectorPP {
    guards: Vec<PatchGuard>,
    verifiers: Vec<CallCountVerifier>,
    _lock: MutexGuard<'static, ()>,
}

impl InjectorPP {
    /// Creates a new InjectorPP instance.
    pub fn new() -> Self {
        let lock = LOCK_FUNCTION.lock();

        Self {
            guards: Vec::new(),
            verifiers: Vec::new(),
            _lock: lock,
        }
    }

    /// Returns a builder to patch the given function.
    pub fn when_called(&mut self, func: *const ()) -> WhenCalledBuilder<'_> {
        let when = WhenCalled::new(func);
        WhenCalledBuilder { lib: self, when }
    }

    pub fn when_called_async<F, T>(&mut self, _: Pin<&mut F>) -> WhenCalledBuilderAsync<'_>
    where
        F: Future<Output = T>,
    {
        let poll_fn: fn(Pin<&mut F>, &mut Context<'_>) -> Poll<T> = <F as Future>::poll;
        let when = WhenCalled::new(func!(poll_fn));
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
    /// Patches the target function to branch to the provided function.
    pub fn will_execute_raw(self, target: *const ()) {
        let guard = self.when.will_execute_guard(target);
        self.lib.guards.push(guard);
    }

    pub fn will_execute(self, fake_pair: (*const (), CallCountVerifier)) {
        let (fake_func, verifier) = fake_pair;
        self.lib.verifiers.push(verifier);
        self.will_execute_raw(func!(fake_func));
    }

    /// Patches the target function to return a fixed boolean.
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
    pub fn will_return_async(self, target: *const ()) {
        let guard = self.when.will_execute_guard(target);
        self.lib.guards.push(guard);
    }
}
