use injectorpp::interface::injector::*;
use std::{fmt::Display, path::Path};

fn foo() {
    println!("foo");
}

async fn simple_async_func_u32_add_one(x: u32) -> u32 {
    x + 1
}

pub fn return_string() -> String {
    return "Hello, world!".to_string();
}

fn complex_generic_multiple_types_func<A: Display, B: Display, C: Display>(
    _a: A,
    _b: B,
    _c: C,
) -> String {
    return "Original value".to_string();
}

#[test]
#[should_panic(
    expected = "Signature mismatch: will_return_boolean requires a function returning bool"
)]
fn test_will_return_boolean_mismatched_type_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(return_string, fn() -> String))
        .will_return_boolean(true);
}

#[test]
#[should_panic(expected = "Signature mismatch")]
fn test_will_execute_when_function_signature_mismatch_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(Path::exists, fn(&Path) -> bool))
        .will_execute(injectorpp::fake!(
            func_type: fn(_path: &str) -> bool,
            returns: true
        ));
}

#[test]
#[should_panic(expected = "Signature mismatch")]
fn test_will_execute_when_generic_function_multiple_types_signature_mismatch_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            complex_generic_multiple_types_func,
            fn(&'static str, bool, i32) -> String
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn(a: &str, b: bool) -> String,
            when: a == "abc" && b == true,
            returns: "Fake value".to_string(),
            times: 1
        ));
}

#[test]
#[should_panic(expected = "Pointer must not be null")]
fn test_will_execute_null_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(foo, fn()))
        .will_execute((
            unsafe {
                FuncPtr::new(std::ptr::null(), {
                    let f: fn() = foo;
                    std::any::type_name_of_val(&f)
                })
            },
            CallCountVerifier::Dummy,
        ));
}

#[tokio::test]
#[should_panic(expected = "Pointer must not be null")]
async fn test_will_return_async_null_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(
            simple_async_func_u32_add_one(u32::default()),
            u32
        ))
        .will_return_async(unsafe { FuncPtr::new(std::ptr::null(), std::any::type_name::<u32>()) });
}

#[tokio::test]
#[should_panic(expected = "Signature mismatch")]
async fn test_will_return_async_mismatched_type_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(
            simple_async_func_u32_add_one(u32::default()),
            u32
        ))
        .will_return_async(injectorpp::async_return!("Test Value".to_string(), String));
}
