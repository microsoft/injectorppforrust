// Thread-local dispatch is available on x86_64 and aarch64. On other architectures,
// InjectorPP uses a global mutex which deadlocks with the barrier-based
// synchronization these tests rely on.
#![cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm"))]

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

use injectorpp::interface::injector::*;

// ============================================================================
// Target functions for thread-safety tests.
// Each must be #[inline(never)] to prevent the compiler from inlining them.
// ============================================================================

#[inline(never)]
fn get_value() -> i32 {
    std::hint::black_box(-1)
}

#[inline(never)]
fn get_other_value() -> i32 {
    std::hint::black_box(-2)
}

#[inline(never)]
fn is_enabled() -> bool {
    std::hint::black_box(false)
}

#[inline(never)]
fn add(a: i32, b: i32) -> i32 {
    std::hint::black_box(a + b)
}

#[inline(never)]
fn do_work() {
    let _ = std::hint::black_box(42u32);
}

// ============================================================================
// Basic thread isolation tests
// ============================================================================

/// Two threads fake the same function with different return values.
/// Each thread should see only its own fake.
#[test]
fn test_two_threads_same_function_different_values() {
    let result1 = Arc::new(AtomicI32::new(0));
    let result2 = Arc::new(AtomicI32::new(0));
    let barrier = Arc::new(Barrier::new(2));

    let r1 = result1.clone();
    let b1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 100
            ));
        b1.wait();
        r1.store(get_value(), Ordering::SeqCst);
        b1.wait();
    });

    let r2 = result2.clone();
    let b2 = barrier.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 200
            ));
        b2.wait();
        r2.store(get_value(), Ordering::SeqCst);
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert_eq!(result1.load(Ordering::SeqCst), 100);
    assert_eq!(result2.load(Ordering::SeqCst), 200);
}

/// A thread without any fake should see the original function behavior,
/// even when another thread has an active fake for the same function.
#[test]
fn test_thread_without_fake_sees_original() {
    let faked_result = Arc::new(AtomicI32::new(0));
    let unfaked_result = Arc::new(AtomicI32::new(0));
    let barrier = Arc::new(Barrier::new(2));

    let fr = faked_result.clone();
    let b1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 999
            ));
        b1.wait();
        fr.store(get_value(), Ordering::SeqCst);
        b1.wait();
    });

    let ur = unfaked_result.clone();
    let b2 = barrier.clone();
    let h2 = thread::spawn(move || {
        // This thread does NOT fake get_value
        b2.wait();
        ur.store(get_value(), Ordering::SeqCst);
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert_eq!(faked_result.load(Ordering::SeqCst), 999);
    assert_eq!(unfaked_result.load(Ordering::SeqCst), -1); // original
}

/// When one thread drops its injector, another thread's fake
/// for the same function should remain active.
#[test]
fn test_drop_on_one_thread_does_not_affect_another() {
    let result_after_other_drop = Arc::new(AtomicI32::new(0));
    let barrier_setup = Arc::new(Barrier::new(2));
    let drop_done = Arc::new(AtomicBool::new(false));
    let verified = Arc::new(AtomicBool::new(false));

    let bs = barrier_setup.clone();
    let dd = drop_done.clone();
    let vf = verified.clone();
    let h1 = thread::spawn(move || {
        {
            let mut injector = InjectorPP::new();
            injector
                .when_called(injectorpp::func!(fn(get_value)() -> i32))
                .will_execute(injectorpp::fake!(
                    func_type: fn() -> i32,
                    returns: 111
                ));
            bs.wait();
            // drop injector here
        }
        dd.store(true, Ordering::SeqCst);
        // Wait for thread 2 to verify
        while !vf.load(Ordering::SeqCst) {
            thread::yield_now();
        }
    });

    let rad = result_after_other_drop.clone();
    let bs2 = barrier_setup.clone();
    let dd2 = drop_done.clone();
    let vf2 = verified.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 222
            ));
        bs2.wait();
        // Wait for thread 1 to drop its injector
        while !dd2.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        rad.store(get_value(), Ordering::SeqCst);
        vf2.store(true, Ordering::SeqCst);
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert_eq!(result_after_other_drop.load(Ordering::SeqCst), 222);
}

/// Faking a function, dropping the injector, and faking again on the same
/// thread should work correctly each time.
#[test]
fn test_sequential_faking_same_thread() {
    // First fake
    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 10
            ));
        assert_eq!(get_value(), 10);
    }

    // Original restored
    assert_eq!(get_value(), -1);

    // Second fake with different value
    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 20
            ));
        assert_eq!(get_value(), 20);
    }

    // Original restored again
    assert_eq!(get_value(), -1);
}

// ============================================================================
// Stress tests
// ============================================================================

/// Stress test: multiple threads all fake the same function and verify concurrent access
/// doesn't corrupt anything. Each thread uses a fixed return value (42) and
/// calls the function 100 times.
#[test]
fn test_many_threads_concurrent_stress() {
    // ARM32 runs in 32-bit compat on ARM64 CI runners with limited resources
    #[cfg(target_arch = "arm")]
    const THREAD_COUNT: usize = 4;
    #[cfg(not(target_arch = "arm"))]
    const THREAD_COUNT: usize = 20;
    let error_count = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(THREAD_COUNT));

    let handles: Vec<_> = (0..THREAD_COUNT)
        .map(|_| {
            let err = error_count.clone();
            let bar = barrier.clone();

            thread::spawn(move || {
                let mut injector = InjectorPP::new();
                injector
                    .when_called(injectorpp::func!(fn(get_value)() -> i32))
                    .will_execute(injectorpp::fake!(
                        func_type: fn() -> i32,
                        returns: 42
                    ));
                bar.wait();

                // Read multiple times to stress test
                for _ in 0..100 {
                    let actual = get_value();
                    if actual != 42 {
                        err.fetch_add(1, Ordering::SeqCst);
                        return;
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(error_count.load(Ordering::SeqCst), 0);
}

/// Rapid cycles of setup and teardown across multiple threads.
/// Each thread fakes, asserts, and drops the injector 50 times.
#[test]
fn test_rapid_setup_teardown_stress() {
    const ITERATIONS: usize = 50;
    const THREAD_COUNT: usize = 4;
    let error_count = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..THREAD_COUNT)
        .map(|_| {
            let err = error_count.clone();
            thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    let mut injector = InjectorPP::new();
                    injector
                        .when_called(injectorpp::func!(fn(get_value)() -> i32))
                        .will_execute(injectorpp::fake!(
                            func_type: fn() -> i32,
                            returns: 777
                        ));
                    let actual = get_value();
                    if actual != 777 {
                        err.fetch_add(1, Ordering::SeqCst);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(error_count.load(Ordering::SeqCst), 0);
}

// ============================================================================
// Multiple methods tests
// ============================================================================

/// Each thread fakes multiple different functions simultaneously.
#[test]
fn test_multiple_methods_faked_per_thread() {
    let val1_t1 = Arc::new(AtomicI32::new(0));
    let val2_t1 = Arc::new(AtomicI32::new(0));
    let val1_t2 = Arc::new(AtomicI32::new(0));
    let val2_t2 = Arc::new(AtomicI32::new(0));
    let barrier = Arc::new(Barrier::new(2));

    let r1a = val1_t1.clone();
    let r1b = val2_t1.clone();
    let b1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 10
            ));
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 11
            ));
        b1.wait();
        r1a.store(get_value(), Ordering::SeqCst);
        r1b.store(get_other_value(), Ordering::SeqCst);
        b1.wait();
    });

    let r2a = val1_t2.clone();
    let r2b = val2_t2.clone();
    let b2 = barrier.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 20
            ));
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> i32,
                returns: 22
            ));
        b2.wait();
        r2a.store(get_value(), Ordering::SeqCst);
        r2b.store(get_other_value(), Ordering::SeqCst);
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert_eq!(val1_t1.load(Ordering::SeqCst), 10);
    assert_eq!(val2_t1.load(Ordering::SeqCst), 11);
    assert_eq!(val1_t2.load(Ordering::SeqCst), 20);
    assert_eq!(val2_t2.load(Ordering::SeqCst), 22);
}

// ============================================================================
// Boolean method thread isolation
// ============================================================================

/// Boolean function: one thread fakes to true, another sees original (false).
#[test]
fn test_bool_method_thread_isolation() {
    let faked_result = Arc::new(AtomicBool::new(false));
    let unfaked_result = Arc::new(AtomicBool::new(true)); // intentionally wrong default
    let barrier = Arc::new(Barrier::new(2));

    let fr = faked_result.clone();
    let b1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(is_enabled)() -> bool))
            .will_return_boolean(true);
        b1.wait();
        fr.store(is_enabled(), Ordering::SeqCst);
        b1.wait();
    });

    let ur = unfaked_result.clone();
    let b2 = barrier.clone();
    let h2 = thread::spawn(move || {
        // No fake — should see original (false)
        b2.wait();
        ur.store(is_enabled(), Ordering::SeqCst);
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert!(faked_result.load(Ordering::SeqCst));
    assert!(!unfaked_result.load(Ordering::SeqCst));
}

/// Two threads fake the same boolean function with opposite values.
#[test]
fn test_bool_two_threads_opposite_values() {
    let result_true = Arc::new(AtomicBool::new(false));
    let result_false = Arc::new(AtomicBool::new(true));
    let barrier = Arc::new(Barrier::new(2));

    let rt = result_true.clone();
    let b1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(is_enabled)() -> bool))
            .will_return_boolean(true);
        b1.wait();
        rt.store(is_enabled(), Ordering::SeqCst);
        b1.wait();
    });

    let rf = result_false.clone();
    let b2 = barrier.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(is_enabled)() -> bool))
            .will_return_boolean(false);
        b2.wait();
        rf.store(is_enabled(), Ordering::SeqCst);
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert!(result_true.load(Ordering::SeqCst));
    assert!(!result_false.load(Ordering::SeqCst));
}

// ============================================================================
// Closure/will_execute_raw thread isolation
// ============================================================================

/// Two threads use will_execute_raw with different closures for the same function.
#[test]
fn test_will_execute_raw_closure_per_thread() {
    let pre_val = get_value();
    assert_eq!(pre_val, -1);

    let result1 = Arc::new(AtomicI32::new(0));
    let result2 = Arc::new(AtomicI32::new(0));
    let barrier = Arc::new(Barrier::new(2));

    let r1 = result1.clone();
    let b1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute_raw(injectorpp::closure!(|| { 42 }, fn() -> i32));
        b1.wait();
        let val = get_value();
        r1.store(val, Ordering::SeqCst);
        b1.wait();
    });

    let r2 = result2.clone();
    let b2 = barrier.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute_raw(injectorpp::closure!(|| { 84 }, fn() -> i32));
        b2.wait();
        let val = get_value();
        r2.store(val, Ordering::SeqCst);
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert_eq!(result1.load(Ordering::SeqCst), 42);
    assert_eq!(result2.load(Ordering::SeqCst), 84);
}

// ============================================================================
// Functions with parameters
// ============================================================================

/// Two threads fake a function that takes parameters, with different behaviors.
#[test]
fn test_function_with_params_thread_isolation() {
    let result1 = Arc::new(AtomicI32::new(0));
    let result2 = Arc::new(AtomicI32::new(0));
    let barrier = Arc::new(Barrier::new(2));

    let r1 = result1.clone();
    let b1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(add)(i32, i32) -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn(_a: i32, _b: i32) -> i32,
                returns: 1000 // always return 1000 regardless of args
            ));
        b1.wait();
        r1.store(add(3, 4), Ordering::SeqCst);
        b1.wait();
    });

    let r2 = result2.clone();
    let b2 = barrier.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(add)(i32, i32) -> i32))
            .will_execute(injectorpp::fake!(
                func_type: fn(a: i32, b: i32) -> i32,
                returns: a * b // multiply instead of add
            ));
        b2.wait();
        r2.store(add(3, 4), Ordering::SeqCst);
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert_eq!(result1.load(Ordering::SeqCst), 1000);
    assert_eq!(result2.load(Ordering::SeqCst), 12); // 3 * 4
}

// ============================================================================
// Restoration tests
// ============================================================================

/// After all threads dispose their injectors, the original function
/// behavior should be fully restored.
#[test]
fn test_all_threads_dispose_original_restored() {
    let barrier_setup = Arc::new(Barrier::new(3));
    let barrier_verify = Arc::new(Barrier::new(3));
    let error_count = Arc::new(AtomicUsize::new(0));

    // Thread 1: return 100
    let bs1 = barrier_setup.clone();
    let bv1 = barrier_verify.clone();
    let err1 = error_count.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 100));
        bs1.wait();
        if get_other_value() != 100 {
            err1.fetch_add(1, Ordering::SeqCst);
        }
        bv1.wait();
    });

    // Thread 2: return 200
    let bs2 = barrier_setup.clone();
    let bv2 = barrier_verify.clone();
    let err2 = error_count.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 200));
        bs2.wait();
        if get_other_value() != 200 {
            err2.fetch_add(1, Ordering::SeqCst);
        }
        bv2.wait();
    });

    // Thread 3: return 300
    let bs3 = barrier_setup.clone();
    let bv3 = barrier_verify.clone();
    let err3 = error_count.clone();
    let h3 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 300));
        bs3.wait();
        if get_other_value() != 300 {
            err3.fetch_add(1, Ordering::SeqCst);
        }
        bv3.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();
    h3.join().unwrap();

    assert_eq!(error_count.load(Ordering::SeqCst), 0);
    // All threads disposed, original should be restored
    assert_eq!(get_other_value(), -2);
}

// ============================================================================
// Simulated parallel tests
// ============================================================================

/// Simulates parallel test methods — multiple independent "tests" running
/// on different threads, each faking multiple functions with different values.
#[test]
fn test_simulated_parallel_tests_no_interference() {
    let error_count = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(4));

    // Thread 1: get_value=100, get_other_value=101
    let err1 = error_count.clone();
    let bar1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 100));
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 101));
        bar1.wait();
        for _ in 0..50 {
            if get_value() != 100 || get_other_value() != 101 {
                err1.fetch_add(1, Ordering::SeqCst);
                return;
            }
        }
    });

    // Thread 2: get_value=200, get_other_value=201
    let err2 = error_count.clone();
    let bar2 = barrier.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 200));
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 201));
        bar2.wait();
        for _ in 0..50 {
            if get_value() != 200 || get_other_value() != 201 {
                err2.fetch_add(1, Ordering::SeqCst);
                return;
            }
        }
    });

    // Thread 3: get_value=300, get_other_value=301
    let err3 = error_count.clone();
    let bar3 = barrier.clone();
    let h3 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 300));
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 301));
        bar3.wait();
        for _ in 0..50 {
            if get_value() != 300 || get_other_value() != 301 {
                err3.fetch_add(1, Ordering::SeqCst);
                return;
            }
        }
    });

    // Thread 4: get_value=400, get_other_value=401
    let err4 = error_count.clone();
    let bar4 = barrier.clone();
    let h4 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 400));
        injector
            .when_called(injectorpp::func!(fn(get_other_value)() -> i32))
            .will_execute(injectorpp::fake!(func_type: fn() -> i32, returns: 401));
        bar4.wait();
        for _ in 0..50 {
            if get_value() != 400 || get_other_value() != 401 {
                err4.fetch_add(1, Ordering::SeqCst);
                return;
            }
        }
    });

    h1.join().unwrap();
    h2.join().unwrap();
    h3.join().unwrap();
    h4.join().unwrap();

    assert_eq!(error_count.load(Ordering::SeqCst), 0);
}

// ============================================================================
// Void function tests
// ============================================================================

/// Thread isolation for void functions: one thread replaces with a side-effect
/// tracker, another sees original behavior.
#[test]
fn test_void_function_thread_isolation() {
    static THREAD_CALLED: AtomicBool = AtomicBool::new(false);
    let barrier = Arc::new(Barrier::new(2));

    let b1 = barrier.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(do_work)()))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> (),
                assign: { THREAD_CALLED.store(true, Ordering::SeqCst); }
            ));
        b1.wait();
        do_work();
        b1.wait();
    });

    let b2 = barrier.clone();
    let h2 = thread::spawn(move || {
        // No fake — calls original
        b2.wait();
        do_work();
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert!(THREAD_CALLED.load(Ordering::SeqCst));
}

// ============================================================================
// Preventer compatibility test
// ============================================================================

/// Verify that InjectorPP::prevent() still works (even if it's a no-op on x86_64).
#[test]
fn test_preventer_still_works() {
    let result = Arc::new(AtomicI32::new(0));
    let barrier = Arc::new(Barrier::new(2));

    let r = result.clone();
    let b1 = barrier.clone();
    let h = thread::spawn(move || {
        let _guard = InjectorPP::prevent();
        b1.wait();
        r.store(get_value(), Ordering::SeqCst);
        b1.wait();
    });

    // Main thread fakes the function
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn(get_value)() -> i32))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> i32,
            returns: 555
        ));
    barrier.wait();
    // Main thread should see its fake
    assert_eq!(get_value(), 555);
    barrier.wait();

    h.join().unwrap();

    // The other thread should see original (thread-local dispatch isolates)
    assert_eq!(result.load(Ordering::SeqCst), -1);
}

// ============================================================================
// Edge case: same function faked & restored repeatedly across threads
// ============================================================================

/// Multiple threads fake and restore the same function repeatedly.
/// After all threads finish, the original behavior should be intact.
#[test]
fn test_repeated_fake_restore_across_threads() {
    const ITERATIONS: usize = 20;
    const THREAD_COUNT: usize = 4;
    let error_count = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..THREAD_COUNT)
        .map(|_| {
            let err = error_count.clone();
            thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    {
                        let mut injector = InjectorPP::new();
                        injector
                            .when_called(injectorpp::func!(fn(get_value)() -> i32))
                            .will_execute(injectorpp::fake!(
                                func_type: fn() -> i32,
                                returns: 999
                            ));
                        let actual = get_value();
                        if actual != 999 {
                            err.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    // After drop, if no other thread has a fake, original should be callable.
                    // (We don't assert original here because another thread might have a live fake.)
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(error_count.load(Ordering::SeqCst), 0);
    // All threads done, original should be restored
    assert_eq!(get_value(), -1);
}

// ============================================================================
// String return value thread isolation (via closure)
// ============================================================================

#[inline(never)]
fn get_greeting() -> String {
    "hello".to_string()
}

/// Two threads return different strings from the same function.
#[test]
fn test_string_return_thread_isolation() {
    let barrier = Arc::new(Barrier::new(2));
    let error_count = Arc::new(AtomicUsize::new(0));

    let b1 = barrier.clone();
    let e1 = error_count.clone();
    let h1 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_greeting)() -> String))
            .will_execute_raw(injectorpp::closure!(|| { "from_thread_1".to_string() }, fn() -> String));
        b1.wait();
        if get_greeting() != "from_thread_1" {
            e1.fetch_add(1, Ordering::SeqCst);
        }
        b1.wait();
    });

    let b2 = barrier.clone();
    let e2 = error_count.clone();
    let h2 = thread::spawn(move || {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(get_greeting)() -> String))
            .will_execute_raw(injectorpp::closure!(|| { "from_thread_2".to_string() }, fn() -> String));
        b2.wait();
        if get_greeting() != "from_thread_2" {
            e2.fetch_add(1, Ordering::SeqCst);
        }
        b2.wait();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert_eq!(error_count.load(Ordering::SeqCst), 0);
}
