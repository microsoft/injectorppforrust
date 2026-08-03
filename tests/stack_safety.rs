// Test that JIT memory allocation does not interfere with stack growth.
//
// The old x86_64 Windows implementation of `allocate_jit_memory` scanned linearly
// from `func_addr - 2GB`, which could allocate memory in/near the stack region,
// disrupting the stack guard page and causing STATUS_STACK_OVERFLOW (0xc00000fd)
// during parallel test execution.
//
// These tests verify that after patching functions, deep stack usage still works.
#![cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm"))]

use std::sync::{Arc, Barrier};
use std::thread;

use injectorpp::interface::injector::*;

#[inline(never)]
fn get_value_stack_test() -> i32 {
    std::hint::black_box(1)
}

#[inline(never)]
fn get_other_value_stack_test() -> i32 {
    std::hint::black_box(2)
}

#[inline(never)]
fn is_enabled_stack_test() -> bool {
    std::hint::black_box(false)
}

/// Consume stack space via recursion. Each frame is ~256 bytes due to the array.
/// At depth 2000, this uses ~512KB of the thread's stack.
#[inline(never)]
fn consume_stack(depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    // Force the compiler to keep a sizable stack frame.
    let buf = std::hint::black_box([0u8; 256]);
    let result = consume_stack(depth - 1);
    std::hint::black_box(buf[0] as u64) + result
}

/// After patching a function, verify that deep recursion still works.
/// With the old buggy JIT allocator, the stack guard page could be disrupted,
/// causing a stack overflow even at moderate recursion depths.
#[test]
fn test_stack_growth_works_after_patching() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn(get_value_stack_test)() -> i32))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> i32,
            returns: 42
        ));

    // Verify the fake works
    assert_eq!(get_value_stack_test(), 42);

    // Now do deep recursion — this should NOT cause STATUS_STACK_OVERFLOW.
    // If JIT memory was allocated in the stack region (old bug), this would crash.
    let result = consume_stack(2000);
    assert!(result > 0, "Deep recursion should succeed after patching");
}

/// Multiple threads concurrently patch different functions and then exercise
/// deep stack usage. This replicates the conditions of the original crash:
/// parallel tests + injectorpp patching + significant stack consumption.
#[test]
fn test_concurrent_patching_with_deep_stack_usage() {
    let thread_count = 8;
    let barrier = Arc::new(Barrier::new(thread_count));
    let mut handles = Vec::new();

    for i in 0..thread_count {
        let b = barrier.clone();
        handles.push(thread::spawn(move || {
            let mut injector = InjectorPP::new();

            // Each thread patches a function
            match i % 3 {
                0 => {
                    injector
                        .when_called(injectorpp::func!(fn(get_value_stack_test)() -> i32))
                        .will_execute(injectorpp::fake!(
                            func_type: fn() -> i32,
                            returns: 100
                        ));
                }
                1 => {
                    injector
                        .when_called(injectorpp::func!(fn(get_other_value_stack_test)() -> i32))
                        .will_execute(injectorpp::fake!(
                            func_type: fn() -> i32,
                            returns: 200
                        ));
                }
                _ => {
                    injector
                        .when_called(injectorpp::func!(fn(is_enabled_stack_test)() -> bool))
                        .will_return_boolean(true);
                }
            };

            // Synchronize: all threads have patched before anyone recurses
            b.wait();

            // Deep recursion — would crash if JIT memory disrupted the stack guard page
            let result = consume_stack(1500);
            assert!(
                result > 0,
                "Thread {i} should complete deep recursion without stack overflow"
            );
        }));
    }

    for (i, h) in handles.into_iter().enumerate() {
        h.join()
            .unwrap_or_else(|_| panic!("Thread {i} panicked — possible stack overflow"));
    }
}
