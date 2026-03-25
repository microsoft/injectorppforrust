// Regression tests for function restoration after mock is dropped (x86_64 Windows).
//
// In v0.5.0, thread-local dispatch permanently patches functions: after a mock
// is dropped, the JIT dispatcher jump instruction remains at the function's entry
// point. While functionally correct (the dispatcher returns the original behavior
// via a trampoline), this has two problems:
//
// 1. **Stack overhead**: Each call through the dispatcher temporarily pushes extra
//    stack for get_thread_target + catch_unwind + TLS lookup. In deep call chains
//    close to the stack limit, this can trigger stack overflow. This was observed
//    in the acs_media_sdk test suite where test #209/503 crashed with
//    STATUS_STACK_OVERFLOW (0xC00000FD) when running sequentially.
//
// 2. **Resource leak**: JIT memory (dispatcher + trampoline) is never freed,
//    and registry entries accumulate across tests.
//
// The fix restores original function bytes on x86_64 when ref_count drops to 0,
// and frees associated JIT memory.
#![cfg(target_arch = "x86_64")]
#![cfg(target_os = "windows")]

use injectorpp::interface::injector::*;

#[inline(never)]
fn recursive_func(depth: u32) -> u32 {
    if depth == 0 {
        return 0;
    }
    std::hint::black_box(recursive_func(depth - 1)) + 1
}

#[inline(never)]
fn fake_recursive(_depth: u32) -> u32 {
    0
}

/// Read the first `n` bytes of machine code at a function's entry point.
unsafe fn read_func_bytes(func_ptr: *const u8, n: usize) -> Vec<u8> {
    std::slice::from_raw_parts(func_ptr, n).to_vec()
}

/// Test that the function's machine code is fully restored after mock is dropped.
///
/// Without the fix: the entry point remains a JMP to the dispatcher (0xE9 ...)
/// With the fix: the original prologue bytes are restored
#[test]
fn test_function_bytes_restored_after_drop() {
    let func_ptr = recursive_func as *const u8;

    // Read original bytes before any patching
    let original_bytes = unsafe { read_func_bytes(func_ptr, 16) };

    // The first byte should NOT be 0xE9 (JMP rel32) before patching
    assert_ne!(
        original_bytes[0], 0xE9,
        "Function should not start with JMP before patching"
    );

    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(recursive_func)(u32) -> u32))
            .will_execute_raw(injectorpp::func!(fn(fake_recursive)(u32) -> u32));

        // While patched, the first bytes should be a JMP (0xE9 for rel32)
        let patched_bytes = unsafe { read_func_bytes(func_ptr, 16) };
        assert_eq!(
            patched_bytes[0], 0xE9,
            "Patched function should start with JMP rel32 (0xE9), got 0x{:02X}",
            patched_bytes[0]
        );
    }

    // After drop, original bytes should be restored
    let restored_bytes = unsafe { read_func_bytes(func_ptr, 16) };
    assert_eq!(
        restored_bytes, original_bytes,
        "Function bytes should be fully restored after mock is dropped.\n\
         Original:  {:02X?}\n\
         Restored:  {:02X?}",
        original_bytes, restored_bytes
    );
}

/// Test that a function can be correctly patched, dropped, and re-patched
/// multiple times. This verifies that registry cleanup allows fresh re-patching.
#[test]
fn test_repeated_patch_and_restore_cycles() {
    let func_ptr = recursive_func as *const u8;
    let original_bytes = unsafe { read_func_bytes(func_ptr, 16) };

    for i in 0..20 {
        // Before patching: original bytes
        let before = unsafe { read_func_bytes(func_ptr, 16) };
        assert_eq!(
            before, original_bytes,
            "Cycle {}: bytes should match original before patching",
            i
        );

        {
            let mut injector = InjectorPP::new();
            injector
                .when_called(injectorpp::func!(fn(recursive_func)(u32) -> u32))
                .will_execute_raw(injectorpp::func!(fn(fake_recursive)(u32) -> u32));

            // While patched: should have JMP
            let during = unsafe { read_func_bytes(func_ptr, 5) };
            assert_eq!(during[0], 0xE9, "Cycle {}: should be patched with JMP", i);

            // Mock should be active
            assert_eq!(recursive_func(3), 0, "Cycle {}: mock should return 0", i);
        }

        // After drop: original bytes restored
        let after = unsafe { read_func_bytes(func_ptr, 16) };
        assert_eq!(
            after, original_bytes,
            "Cycle {}: bytes should be restored after drop",
            i
        );
        assert_eq!(
            recursive_func(3),
            3,
            "Cycle {}: function should work normally",
            i
        );
    }
}

/// Verify behavior: the function returns correct values before, during, and
/// after mocking, including on a thread with limited stack.
#[test]
fn test_function_behavior_across_mock_lifecycle() {
    assert_eq!(recursive_func(10), 10, "Should work before patching");

    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn(recursive_func)(u32) -> u32))
            .will_execute_raw(injectorpp::func!(fn(fake_recursive)(u32) -> u32));
        assert_eq!(recursive_func(10), 0, "Should return mock value during patch");
    }

    assert_eq!(recursive_func(10), 10, "Should return original value after drop");

    // Deep recursion should also work after restoration
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || recursive_func(2000))
        .unwrap();
    assert_eq!(
        handle.join().unwrap(),
        2000,
        "Deep recursion should work after restoration"
    );
}
