/// Issue #73 variant: func_info prefix should also be checked.
use injectorpp::interface::injector::*;

fn foo(_s: &str) -> &'static str {
    "hello"
}

fn main() {
    let _f = injectorpp::func!(func_info: fn (foo)(&str) -> &str);
}
