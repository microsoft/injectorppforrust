/// Issue #73: func! must reject bare reference returns when the actual function
/// has a different (e.g., 'static) return lifetime.
///
/// This is the exact pattern from issue #73: foo returns &'static str but
/// the user writes &str, which allows the fake to return a dangling reference.
use injectorpp::interface::injector::*;

fn foo(_s: &str) -> &'static str {
    "hello"
}

fn main() {
    let _f = injectorpp::func!(fn (foo)(&str) -> &str);
}
