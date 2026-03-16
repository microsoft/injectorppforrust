/// Tests for issue #73: compile-time lifetime safety check in func! macro.
///
/// These tests verify that:
/// 1. Mismatched lifetimes in bare reference returns are rejected at compile time
/// 2. Correct usage patterns continue to compile and work

#[test]
fn issue73_compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/issue73_*.rs");
}
