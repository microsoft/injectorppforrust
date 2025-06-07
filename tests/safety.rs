use injectorpp::interface::injector::*;

fn foo() {
    println!("foo");
}

async fn simple_async_func_u32_add_one(x: u32) -> u32 {
    x + 1
}

#[test]
#[should_panic(expected = "Pointer must not be null")]
fn test_will_execute_raw_null_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(foo))
        .will_execute_raw(injectorpp::func!(std::ptr::null()));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_will_execute_raw_without_provenance_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(foo))
        .will_execute_raw(injectorpp::func!(std::ptr::without_provenance(0x123456789)));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_will_execute_raw_misaligned_pointer_sub_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(foo))
        .will_execute_raw(injectorpp::func!((foo as *const ()).wrapping_byte_sub(1)));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_will_execute_raw_misaligned_pointer_add_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(foo))
        .will_execute_raw(injectorpp::func!((foo as *const ()).wrapping_byte_add(1)));
}

#[test]
#[should_panic(expected = "Pointer must not be null")]
fn test_will_execute_null_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector.when_called(injectorpp::func!(foo)).will_execute((
        unsafe { FuncPtr::new(std::ptr::null()) },
        CallCountVerifier::Dummy,
    ));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_will_execute_without_provenance_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector.when_called(injectorpp::func!(foo)).will_execute((
        unsafe { FuncPtr::new(std::ptr::without_provenance(0x123456789)) },
        CallCountVerifier::Dummy,
    ));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_will_execute_misaligned_pointer_sub_should_panic() {
    let mut injector = InjectorPP::new();
    injector.when_called(injectorpp::func!(foo)).will_execute((
        unsafe { FuncPtr::new((foo as *const ()).wrapping_byte_sub(1)) },
        CallCountVerifier::Dummy,
    ));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_will_execute_misaligned_pointer_add_should_panic() {
    let mut injector = InjectorPP::new();
    injector.when_called(injectorpp::func!(foo)).will_execute((
        unsafe { FuncPtr::new((foo as *const ()).wrapping_byte_add(1)) },
        CallCountVerifier::Dummy,
    ));
}

#[test]
#[should_panic(expected = "Pointer must not be null")]
fn test_when_called_null_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(std::ptr::null()))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> (),
            returns: ()
        ));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_when_called_without_provenance_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(std::ptr::without_provenance(0x123456789)))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> (),
            returns: ()
        ));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_when_called_misaligned_pointer_sub_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!((foo as *const ()).wrapping_byte_sub(1)))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> (),
            returns: ()
        ));
}

#[test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
fn test_when_called_misaligned_pointer_add_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!((foo as *const ()).wrapping_byte_add(1)))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> (),
            returns: ()
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
        .will_return_async(unsafe { FuncPtr::new(std::ptr::null()) });
}

#[tokio::test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
async fn test_will_return_async_without_provenance_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(simple_async_func_u32_add_one(
            u32::default()
        )))
        .will_return_async(unsafe { FuncPtr::new(std::ptr::without_provenance(0x123456789)) });
}

#[tokio::test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
async fn test_will_return_async_misaligned_pointer_sub_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(simple_async_func_u32_add_one(
            u32::default()
        )))
        .will_return_async(unsafe { FuncPtr::new((foo as *const ()).wrapping_byte_sub(1)) });
}

#[tokio::test]
#[should_panic(expected = "Pointer has insufficient alignment for function pointer")]
async fn test_will_return_async_misaligned_pointer_add_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(simple_async_func_u32_add_one(
            u32::default()
        )))
        .will_return_async(unsafe { FuncPtr::new((foo as *const ()).wrapping_byte_add(1)) });
}
