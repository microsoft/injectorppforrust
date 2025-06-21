use injectorpp::interface::injector::*;

fn foo() {
    println!("foo");
}

async fn simple_async_func_u32_add_one(x: u32) -> u32 {
    x + 1
}

pub fn return_string() -> String {
    return "Hello, world!".to_string();
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
#[should_panic(expected = "Pointer must not be null")]
fn test_will_execute_null_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(foo, fn()))
        .will_execute((
            unsafe { FuncPtr::new(std::ptr::null(), std::any::type_name_of_val(&foo)) },
            CallCountVerifier::Dummy,
        ));
}

#[tokio::test]
#[should_panic(expected = "Pointer must not be null")]
async fn test_will_return_async_null_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(simple_async_func_u32_add_one(
            u32::default()
        )))
        .will_return_async(unsafe {
            FuncPtr::new(
                std::ptr::null(),
                std::any::type_name_of_val(&simple_async_func_u32_add_one),
            )
        });
}
