/// Lifetime check also applies when using the func_info: prefix syntax.
use injectorpp::interface::injector::*;

fn foo(_s: &str) -> &'static str {
    "hello"
}

fn main() {
    let _f = injectorpp::func!(func_info: fn (foo)(&str) -> &str);
}
