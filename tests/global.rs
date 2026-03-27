use injectorpp::interface::injector::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;

// ---- Helper functions for testing ----
// These use `core::hint::black_box` to ensure each function's compiled code is
// at least 16 bytes. On ARM, the PatchGuard uses 12-byte patches; tiny functions
// placed adjacently by the linker would overlap when patched simultaneously.

#[inline(never)]
fn global_test_func() -> i32 {
    core::hint::black_box(core::hint::black_box(21) + core::hint::black_box(21))
}

#[inline(never)]
fn global_test_func_bool() -> bool {
    core::hint::black_box(!core::hint::black_box(true))
}

#[inline(never)]
fn global_add(a: i32, b: i32) -> i32 {
    core::hint::black_box(core::hint::black_box(a) + core::hint::black_box(b))
}

#[inline(never)]
fn global_multiply(a: i32, b: i32) -> i32 {
    core::hint::black_box(core::hint::black_box(a) * core::hint::black_box(b))
}

// ---- Tests ----

/// Verifies that a global fake using `will_execute` (fake! macro) is visible from a spawned thread.
#[test]
fn test_global_fake_visible_from_spawned_thread() {
    let mut injector = InjectorPP::new_global();
    injector
        .when_called(injectorpp::func!(fn (global_test_func)() -> i32))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> i32,
            returns: 99,
            times: 2
        ));

    assert_eq!(global_test_func(), 99);

    let handle = thread::spawn(global_test_func);
    assert_eq!(handle.join().unwrap(), 99);
}

/// Verifies that `will_return_boolean` in global mode is visible from a spawned thread.
#[test]
fn test_global_fake_boolean_visible_from_spawned_thread() {
    let mut injector = InjectorPP::new_global();
    injector
        .when_called(injectorpp::func!(fn (global_test_func_bool)() -> bool))
        .will_return_boolean(true);

    assert!(global_test_func_bool());

    let handle = thread::spawn(global_test_func_bool);
    assert!(handle.join().unwrap());
}

/// Verifies that `will_execute_raw` (named function) in global mode works across threads.
#[test]
fn test_global_fake_will_execute_raw_cross_thread() {
    fn fake_add(_a: i32, _b: i32) -> i32 {
        1000
    }

    let mut injector = InjectorPP::new_global();
    injector
        .when_called(injectorpp::func!(fn (global_add)(i32, i32) -> i32))
        .will_execute_raw(injectorpp::func!(fn (fake_add)(i32, i32) -> i32));

    assert_eq!(global_add(1, 2), 1000);

    let handle = thread::spawn(|| global_add(10, 20));
    assert_eq!(handle.join().unwrap(), 1000);
}

/// Verifies that a closure-based fake in global mode works across threads.
#[test]
fn test_global_fake_closure_cross_thread() {
    let mut injector = InjectorPP::new_global();
    injector
        .when_called(injectorpp::func!(fn (global_multiply)(i32, i32) -> i32))
        .will_execute_raw(injectorpp::closure!(|_a: i32, _b: i32| -> i32 { 777 }, fn(i32, i32) -> i32));

    assert_eq!(global_multiply(3, 4), 777);

    let handle = thread::spawn(|| global_multiply(5, 6));
    assert_eq!(handle.join().unwrap(), 777);
}

/// Verifies that multiple functions can be faked in the same global injector.
#[test]
fn test_global_multiple_fakes_in_same_injector() {
    let mut injector = InjectorPP::new_global();

    injector
        .when_called(injectorpp::func!(fn (global_test_func)() -> i32))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> i32,
            returns: 111,
            times: 1
        ));

    injector
        .when_called(injectorpp::func!(fn (global_test_func_bool)() -> bool))
        .will_return_boolean(true);

    assert_eq!(global_test_func(), 111);
    assert!(global_test_func_bool());
}

/// Verifies that after a global injector is dropped, the original function is restored.
#[test]
fn test_global_fake_restores_original_after_drop() {
    {
        let mut injector = InjectorPP::new_global();
        injector
            .when_called(injectorpp::func!(fn (global_test_func)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 555,
                times: 1
            ));

        assert_eq!(global_test_func(), 555);
        // injector drops here
    }

    // Original function should be restored
    assert_eq!(global_test_func(), 42);
}

/// Verifies that a global fake is visible from multiple concurrently spawned threads.
#[test]
fn test_global_fake_visible_from_many_threads() {
    let mut injector = InjectorPP::new_global();
    injector
        .when_called(injectorpp::func!(fn (global_test_func)() -> i32))
        .will_execute_raw(injectorpp::closure!(|| -> i32 { 42_000 }, fn() -> i32));

    let counter = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();

    for _ in 0..8 {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            let result = global_test_func();
            if result == 42_000 {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All 8 threads should have seen the global fake
    assert_eq!(counter.load(Ordering::SeqCst), 8);
}

/// Verifies that `will_execute_raw_unchecked` in global mode works across threads.
#[test]
fn test_global_fake_unchecked_cross_thread() {
    fn fake_func() -> i32 {
        9999
    }

    let mut injector = InjectorPP::new_global();
    unsafe {
        injector
            .when_called(injectorpp::func_unchecked!(global_test_func))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_func));
    }

    assert_eq!(global_test_func(), 9999);

    let handle = thread::spawn(global_test_func);
    assert_eq!(handle.join().unwrap(), 9999);
}

/// Verifies that `new()` (thread-local mode) still works correctly — fakes are NOT visible
/// from spawned threads (default 0.5.0 behavior).
#[test]
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm"))]
fn test_thread_local_mode_not_visible_from_spawned_thread() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (global_add)(i32, i32) -> i32))
        .will_execute_raw(injectorpp::closure!(|_a: i32, _b: i32| -> i32 { 9999 }, fn(i32, i32) -> i32));

    // Test thread sees the fake
    assert_eq!(global_add(1, 2), 9999);

    // Spawned thread should NOT see the fake (thread-local mode)
    let handle = thread::spawn(|| global_add(1, 2));
    assert_eq!(handle.join().unwrap(), 3);
}

/// Verifies that a global fake with call-count verification works correctly
/// when calls come from multiple threads.
#[test]
fn test_global_fake_call_count_across_threads() {
    let mut injector = InjectorPP::new_global();
    injector
        .when_called(injectorpp::func!(fn (global_test_func)() -> i32))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> i32,
            returns: 50,
            times: 4
        ));

    // 1 call from test thread
    assert_eq!(global_test_func(), 50);

    // 3 calls from spawned threads
    let mut handles = Vec::new();
    for _ in 0..3 {
        handles.push(thread::spawn(|| {
            assert_eq!(global_test_func(), 50);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    // Verifier checks times:4 on drop — this test passes only if exactly 4 calls were made.
}
