/// Exact reproduction from issue #73.
/// The user creates a fake that returns a borrowed `s` as if it were &'static str,
/// causing use-after-free.
use injectorpp::interface::injector::*;

#[inline(never)]
fn foo(_s: &str) -> &'static str {
    "abc"
}

fn main() {
    let mut injector = InjectorPP::new();

    injector
        .when_called(injectorpp::func!(fn (foo)(&str) -> &str))
        .will_execute(injectorpp::fake!(
            func_type: fn(s: &str) -> &str,
            returns: s
        ));

    let s = {
        let s = String::from("foo");
        foo(&s)
    };

    println!("{s}");
}
