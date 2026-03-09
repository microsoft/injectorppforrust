use injectorpp::interface::injector::*;

pub fn returns_false() -> bool {
    false
}

pub fn returns_false_in_scope() -> bool {
    false
}

fn complex_generic_multiple_types_func_return_false<A, B, C>(_a: A, _b: B, _c: C) -> bool {
    false
}

fn call_with_another_life_time<'a>(s: &'a str) -> bool {
    // Here `'a` is in scope, so you *can* name it in your turbofish:
    let result: bool =
        complex_generic_multiple_types_func_return_false::<i32, bool, &'a str>(42, false, s);

    result
}

#[test]
fn test_will_return_boolean_when_in_scope_should_restore() {
    assert!(!returns_false_in_scope());

    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn (returns_false_in_scope)() -> bool))
            .will_return_boolean(true);

        let result = returns_false_in_scope();
        assert!(result);
    }

    let restored = returns_false_in_scope();

    assert!(!restored);
}

#[test]
fn test_will_return_boolean_when_fake_return_true_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (returns_false)() -> bool))
        .will_return_boolean(true);

    let result = returns_false();

    assert!(result);
}

#[test]
fn test_will_return_boolean_when_fake_return_false_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (returns_false)() -> bool))
        .will_return_boolean(false);

    let result = returns_false();

    assert!(!result);
}

#[test]
fn test_will_return_boolean_when_fake_complex_generic_function_multiple_types_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (complex_generic_multiple_types_func_return_false)(i32, bool, &'static str) -> bool
        ))
        .will_return_boolean(true);

    let result = complex_generic_multiple_types_func_return_false(1, false, "test string");

    assert!(result);
}

#[test]
fn test_will_return_boolean_when_fake_complex_generic_function_multiple_types_another_life_time_should_success(
) {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (complex_generic_multiple_types_func_return_false)(i32, bool, &'static str) -> bool
        ))
        .will_return_boolean(true);

    let my_str = String::from("hello");
    let result = call_with_another_life_time(&my_str);

    assert!(result);
}
