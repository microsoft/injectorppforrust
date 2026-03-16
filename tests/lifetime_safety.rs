// Compile-time lifetime safety tests for the func! macro.
//
// These tests verify that:
// - Mismatched lifetimes in bare reference returns are rejected at compile time
// - Correct usage patterns (explicit lifetimes, non-reference returns) continue to work

use injectorpp::interface::injector::*;

// ======================================================================
// Compile-fail tests: these must NOT compile
// Uses rustc directly to avoid fragile stderr snapshot matching.
// ======================================================================

/// Helper: tries to compile a source file and returns whether it succeeded.
/// Returns None if build artifacts can't be found (e.g. cross-compilation).
fn try_compile(source_path: &str) -> Option<bool> {
    let rlib = find_file(&["target/debug/deps", "target/debug"], "libinjectorpp", ".rlib")?;
    let ext = if cfg!(windows) { ".dll" } else if cfg!(target_os = "macos") { ".dylib" } else { ".so" };
    let proc_dylib = find_file(&["target/debug/deps", "target/debug"], "injectorpp_macros", ext)?;

    let output = std::process::Command::new("rustc")
        .args([
            "--edition", "2021",
            "--crate-type", "bin",
            "-L", "target/debug/deps",
            "--extern", &format!("injectorpp={}", rlib),
            "--extern", &format!("injectorpp_macros={}", proc_dylib),
            "-o", if cfg!(windows) { "NUL" } else { "/dev/null" },
            source_path,
        ])
        .output()
        .expect("failed to invoke rustc");
    Some(output.status.success())
}

fn find_file(dirs: &[&str], prefix: &str, suffix: &str) -> Option<String> {
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(prefix) && name.ends_with(suffix) {
                    return Some(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

#[test]
fn static_str_coerced_to_bare_ref_must_not_compile() {
    match try_compile("tests/compile_fail/static_str_coerced_to_bare_ref.rs") {
        Some(compiled) => assert!(!compiled, "expected compile error: &'static str coerced to bare &str should be rejected"),
        None => eprintln!("skipped: build artifacts not found"),
    }
}

#[test]
fn static_slice_coerced_to_bare_ref_must_not_compile() {
    match try_compile("tests/compile_fail/static_slice_coerced_to_bare_ref.rs") {
        Some(compiled) => assert!(!compiled, "expected compile error: &'static [u8] coerced to bare &[u8] should be rejected"),
        None => eprintln!("skipped: build artifacts not found"),
    }
}

#[test]
fn func_info_prefix_lifetime_mismatch_must_not_compile() {
    match try_compile("tests/compile_fail/func_info_prefix_lifetime_mismatch.rs") {
        Some(compiled) => assert!(!compiled, "expected compile error: lifetime mismatch with func_info: prefix should be rejected"),
        None => eprintln!("skipped: build artifacts not found"),
    }
}

#[test]
fn fake_returns_dangling_reference_must_not_compile() {
    match try_compile("tests/compile_fail/fake_returns_dangling_reference.rs") {
        Some(compiled) => assert!(!compiled, "expected compile error: fake returning dangling reference via lifetime coercion should be rejected"),
        None => eprintln!("skipped: build artifacts not found"),
    }
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
