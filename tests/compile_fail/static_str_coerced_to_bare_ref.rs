/// Bare &str return when function actually returns &'static str.
/// This is the exact pattern from issue #73 that causes use-after-free.
use injectorpp::interface::injector::*;

fn foo(_s: &str) -> &'static str {
    "hello"
}

fn main() {
    let _f = injectorpp::func!(fn (foo)(&str) -> &str);
}
