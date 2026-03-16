/// Compile-time lifetime safety tests for the func! macro.
///
/// These tests verify that:
/// - Mismatched lifetimes in bare reference returns are rejected at compile time
/// - Correct usage patterns (explicit lifetimes, non-reference returns) continue to work

use injectorpp::interface::injector::*;

// ======================================================================
// Compile-fail tests: these must NOT compile (trybuild verifies this)
// ======================================================================

#[test]
fn lifetime_mismatch_compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}

// ======================================================================
// Pass tests: correct usage patterns that must continue to work
// ======================================================================

fn returns_static_str(_s: &str) -> &'static str {
    "hello"
}

fn linked_lifetime_str(s: &str) -> &str {
    s
}

fn returns_option_ref(s: &str) -> Option<&str> {
    Some(s)
}

fn returns_i32(x: i32) -> i32 {
    x + 1
}

fn no_return() {}

/// Explicit &'static str — correct, no check needed.
#[test]
fn pass_explicit_static_lifetime() {
    let _f = injectorpp::func!(fn (returns_static_str)(&str) -> &'static str);
}

/// Explicit &'_ str — user acknowledges linked lifetime, no check.
#[test]
fn pass_explicit_elided_lifetime() {
    let _f = injectorpp::func!(fn (linked_lifetime_str)(&str) -> &'_ str);
}

/// Option<&str> is not a bare reference — no check applied.
#[test]
fn pass_wrapped_reference_return() {
    let _f = injectorpp::func!(fn (returns_option_ref)(&str) -> Option<&str>);
}

/// Non-reference return — no check applied.
#[test]
fn pass_non_reference_return() {
    let _f = injectorpp::func!(fn (returns_i32)(i32) -> i32);
}

/// Unit return — no check applied.
#[test]
fn pass_unit_return() {
    let _f = injectorpp::func!(fn (no_return)());
}

/// Case 2 (explicit type) — not subject to the proc macro check.
#[test]
fn pass_case2_explicit_type() {
    let _f = injectorpp::func!(returns_i32, fn(i32) -> i32);
}
