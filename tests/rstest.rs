//! Tests that injectorpp is compatible with rstest

use injectorpp::interface::injector::*;
use rstest::rstest;

fn foo(input: usize) -> usize {
    input
}

#[rstest]
#[case(0)]
#[case(1)]
fn test_multiple_cases(#[case] x: usize) {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (foo)(usize) -> usize))
        .will_execute(injectorpp::fake!(
            func_type: fn(input: usize) -> usize,
            returns: input + 1,
            times: 1
        ));

    assert_eq!(foo(x), x + 1);
}
