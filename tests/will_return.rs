use injectorpp::interface::injector::*;

pub fn returns_false() -> bool {
    return false;
}

pub fn returns_false_in_scope() -> bool {
    return false;
}

fn complex_generic_multiple_types_func_return_false<A, B, C>(_a: A, _b: B, _c: C) -> bool {
    return false;
}

fn call_with_another_life_time<'a>(s: &'a str) -> bool {
    // Here `'a` is in scope, so you *can* name it in your turbofish:
    let result: bool =
        complex_generic_multiple_types_func_return_false::<i32, bool, &'a str>(42, false, s);

    result
}

#[test]
fn test_will_return_boolean_when_in_scope_should_restore() {
    assert_eq!(returns_false_in_scope(), false);

    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(returns_false_in_scope, fn() -> bool))
            .will_return_boolean(true);

        let result = returns_false_in_scope();
        assert_eq!(result, true);
    }

    let restored = returns_false_in_scope();

    assert_eq!(restored, false);
}

#[test]
fn test_will_return_boolean_when_fake_return_true_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(returns_false, fn() -> bool))
        .will_return_boolean(true);

    let result = returns_false();

    assert_eq!(result, true);
}

#[test]
fn test_will_return_boolean_when_fake_return_false_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(returns_false, fn() -> bool))
        .will_return_boolean(false);

    let result = returns_false();

    assert_eq!(result, false);
}

#[test]
fn test_will_return_boolean_when_fake_complex_generic_function_multiple_types_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            complex_generic_multiple_types_func_return_false,
            fn(i32, bool, &'static str) -> bool
        ))
        .will_return_boolean(true);

    let result = complex_generic_multiple_types_func_return_false(1, false, "test string");

    assert_eq!(result, true);
}

#[test]
fn test_will_return_boolean_when_fake_complex_generic_function_multiple_types_another_life_time_should_success(
) {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            complex_generic_multiple_types_func_return_false,
            fn(i32, bool, &'static str) -> bool
        ))
        .will_return_boolean(true);

    let my_str = String::from("hello");
    let result = call_with_another_life_time(&my_str);

    assert_eq!(result, true);
}
