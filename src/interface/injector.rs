use crate::injector_core::common::*;
use crate::injector_core::internal::*;
pub use crate::interface::func_ptr::FuncPtr;
pub use crate::interface::macros::__assert_future_output;
pub use crate::interface::verifier::CallCountVerifier;

use std::future::Future;
use std::pin::Pin;
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
struct NoPoisonMutex<T> {
    inner: Mutex<T>,
}

impl<T> NoPoisonMutex<T> {
    /// Create a new mutex.
    const fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
        }
    }

    /// Lock, recovering if the mutex was poisoned.
    fn lock(&self) -> MutexGuard<'_, T> {
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
    ///     .when_called(injectorpp::func!(Path::exists, fn(&Path) -> bool))
    ///     .will_execute_raw(injectorpp::func!(fake_exists, fn(&Path) -> bool));
    ///
    /// assert!(Path::new("/non/existent/path").exists());
    /// ```
    pub fn when_called(&mut self, func: FuncPtr) -> WhenCalledBuilder<'_> {
        let when = WhenCalled::new(func.func_ptr_internal);
        WhenCalledBuilder {
            lib: self,
            when,
            expected_signature: func.signature,
        }
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
    ///         .when_called_async(injectorpp::async_func!(async_add_one(u32::default()), u32))
    ///         .will_return_async(injectorpp::async_return!(123, u32));
    ///
    ///     let result = async_add_one(5).await;
    ///     assert_eq!(result, 123); // The patched value
    /// }
    /// ```
    pub fn when_called_async<F, T>(
        &mut self,
        fake_pair: (Pin<&mut F>, &'static str),
    ) -> WhenCalledBuilderAsync<'_>
    where
        F: Future<Output = T>,
    {
        let poll_fn: fn(Pin<&mut F>, &mut Context<'_>) -> Poll<T> = <F as Future>::poll;
        let when = WhenCalled::new(
            crate::func!(poll_fn, fn(Pin<&mut F>, &mut Context<'_>) -> Poll<T>).func_ptr_internal,
        );

        let signature = fake_pair.1;
        WhenCalledBuilderAsync {
            lib: self,
            when,
            expected_signature: signature,
        }
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
    /// # Safety
    ///
    /// This method is unsafe because it skips type check.
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
    ///
    ///     unsafe {
    ///         injector
    ///             .when_called_async_unchecked(injectorpp::async_func_unchecked!(async_add_one(u32::default())))
    ///             .will_return_async_unchecked(injectorpp::async_return_unchecked!(123, u32));
    ///     }
    ///
    ///     let result = async_add_one(5).await;
    ///     assert_eq!(result, 123); // The patched value
    /// }
    /// ```
    pub unsafe fn when_called_async_unchecked<F, T>(
        &mut self,
        _: Pin<&mut F>,
    ) -> WhenCalledBuilderAsync<'_>
    where
        F: Future<Output = T>,
    {
        let poll_fn: fn(Pin<&mut F>, &mut Context<'_>) -> Poll<T> = <F as Future>::poll;
        let when = WhenCalled::new(
            crate::func!(poll_fn, fn(Pin<&mut F>, &mut Context<'_>) -> Poll<T>).func_ptr_internal,
        );

        WhenCalledBuilderAsync {
            lib: self,
            when,
            expected_signature: "",
        }
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
    expected_signature: &'static str,
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
    ///     .when_called(injectorpp::func!(Path::exists, fn(&Path) -> bool))
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
    ///     .when_called(injectorpp::func!(Path::exists, fn(&Path) -> bool))
    ///     .will_execute_raw(injectorpp::func!(fake_exists, fn(&Path) -> bool));
    ///
    /// assert!(Path::new("/nonexistent").exists());
    /// ```
    pub fn will_execute_raw(self, target: FuncPtr) {
        if target.signature != self.expected_signature {
            panic!(
                "Signature mismatch: expected {:?} but got {:?}",
                self.expected_signature, target.signature
            );
        }

        let guard = self.when.will_execute_guard(target.func_ptr_internal);
        self.lib.guards.push(guard);
    }

    /// Fake the target function to branch to the provided function.
    ///
    /// Allows full customization of the faked function behavior by providing your own function or closure.
    ///
    /// # Parameters
    ///
    /// - `target`: A FuncPtr holds the pointer to the replacement function or closure. Using injectorpp::func_unchecked! or injectorpp::closure! macros is recommended to obtain this pointer.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it skips type check.
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
    ///
    /// unsafe {
    ///     injector
    ///         .when_called(injectorpp::func_unchecked!(Path::exists))
    ///         .will_execute_raw_unchecked(injectorpp::closure!(fake_closure, fn(&Path) -> bool));
    /// }
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
    ///
    /// unsafe {
    ///     injector
    ///         .when_called(injectorpp::func_unchecked!(Path::exists))
    ///         .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_exists));
    /// }
    ///
    /// assert!(Path::new("/nonexistent").exists());
    /// ```
    pub unsafe fn will_execute_raw_unchecked(self, target: FuncPtr) {
        let guard = self.when.will_execute_guard(target.func_ptr_internal);
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
    ///     .when_called(injectorpp::func!(original_func, fn(&mut i32) -> bool))
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
    ///     .when_called(injectorpp::func!(Path::exists, fn(&Path) -> bool))
    ///     .will_return_boolean(true);
    ///
    /// assert!(Path::new("/nonexistent").exists());
    /// ```
    pub fn will_return_boolean(self, value: bool) {
        // Ensure the target function returns a bool
        if !self.expected_signature.trim().ends_with("-> bool") {
            panic!(
                "Signature mismatch: will_return_boolean requires a function returning bool but got {}",
                self.expected_signature
            );
        }

        let guard = self.when.will_return_boolean_guard(value);
        self.lib.guards.push(guard);
    }
}

pub struct WhenCalledBuilderAsync<'a> {
    lib: &'a mut InjectorPP,
    when: WhenCalled,
    expected_signature: &'static str,
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
    ///         .when_called_async(injectorpp::async_func!(async_func_bool(true), bool))
    ///         .will_return_async(injectorpp::async_return!(false, bool));
    ///
    ///     let result = async_func_bool(true).await;
    ///     assert_eq!(result, false);
    /// }
    /// ```
    pub fn will_return_async(self, target: FuncPtr) {
        if target.signature != self.expected_signature {
            panic!(
                "Signature mismatch: expected {:?} but got {:?}",
                self.expected_signature, target.signature
            );
        }

        let guard = self.when.will_execute_guard(target.func_ptr_internal);
        self.lib.guards.push(guard);
    }

    /// Fake the target async function to return a specified async value.
    ///
    /// This method allows you to fake async functions by specifying the return value directly.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it skips type check.
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
    ///     
    ///     unsafe {
    ///         injector
    ///             .when_called_async_unchecked(injectorpp::async_func_unchecked!(async_func_bool(true)))
    ///             .will_return_async_unchecked(injectorpp::async_return_unchecked!(false, bool));
    ///     }
    ///
    ///     let result = async_func_bool(true).await;
    ///     assert_eq!(result, false);
    /// }
    /// ```
    pub unsafe fn will_return_async_unchecked(self, target: FuncPtr) {
        let guard = self.when.will_execute_guard(target.func_ptr_internal);
        self.lib.guards.push(guard);
    }
}
