/// Compile-time lifetime safety tests for the func! macro.
///
/// These tests verify that:
/// - Mismatched lifetimes in bare reference returns are rejected at compile time
/// - Correct usage patterns (explicit lifetimes, non-reference returns) continue to work

use injectorpp::interface::injector::*;

// ======================================================================
// Compile-fail tests: these must NOT compile
// Uses rustc directly to avoid fragile stderr snapshot matching.
// ======================================================================

/// Helper: tries to compile a source file and returns whether it succeeded.
fn try_compile(source_path: &str) -> bool {
    let output = std::process::Command::new("rustc")
        .args([
            "--edition", "2021",
            "--crate-type", "bin",
            "-L", "target/debug/deps",
            "--extern", &format!("injectorpp={}",
                find_rlib("target/debug", "libinjectorpp").unwrap()),
            "--extern", &format!("injectorpp_macros={}",
                find_proc_macro_dylib("target/debug/deps", "injectorpp_macros").unwrap()),
            "-o", if cfg!(windows) { "NUL" } else { "/dev/null" },
            source_path,
        ])
        .output()
        .expect("failed to invoke rustc");
    output.status.success()
}

fn find_rlib(dir: &str, prefix: &str) -> Option<String> {
    std::fs::read_dir(dir).ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with(prefix) && name.ends_with(".rlib")
        })
        .map(|e| e.path().to_string_lossy().to_string())
}

fn find_proc_macro_dylib(dir: &str, prefix: &str) -> Option<String> {
    let ext = if cfg!(windows) { ".dll" } else if cfg!(target_os = "macos") { ".dylib" } else { ".so" };
    std::fs::read_dir(dir).ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with(prefix) && name.ends_with(ext)
        })
        .map(|e| e.path().to_string_lossy().to_string())
}

#[test]
fn static_str_coerced_to_bare_ref_must_not_compile() {
    assert!(
        !try_compile("tests/compile_fail/static_str_coerced_to_bare_ref.rs"),
        "expected compile error: &'static str coerced to bare &str should be rejected"
    );
}

#[test]
fn static_slice_coerced_to_bare_ref_must_not_compile() {
    assert!(
        !try_compile("tests/compile_fail/static_slice_coerced_to_bare_ref.rs"),
        "expected compile error: &'static [u8] coerced to bare &[u8] should be rejected"
    );
}

#[test]
fn func_info_prefix_lifetime_mismatch_must_not_compile() {
    assert!(
        !try_compile("tests/compile_fail/func_info_prefix_lifetime_mismatch.rs"),
        "expected compile error: lifetime mismatch with func_info: prefix should be rejected"
    );
}

#[test]
fn issue73_full_scenario_must_not_compile() {
    assert!(
        !try_compile("tests/compile_fail/issue73_full_scenario.rs"),
        "expected compile error: the exact issue #73 scenario (func + fake + use-after-free) should be rejected"
    );
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
