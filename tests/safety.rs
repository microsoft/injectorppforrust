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
#[should_panic(expected = "Pointer must not be null")]
fn test_will_execute_null_pointer_should_panic() {
    let mut injector = InjectorPP::new();
    injector.when_called(injectorpp::func!(foo)).will_execute((
        unsafe { FuncPtr::new(std::ptr::null()) },
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

#[test]
fn test_multi_thread_scenario() {
    // fire off a worker that continuously calls `foo()`
    std::thread::spawn(|| loop {
        foo();
    });

    // Do a finite number of patch/unpatch cycles—if none of these crash,
    // our multi-thread “race” is safe.
    const ITERATIONS: usize = 1000;
    for _ in 0..ITERATIONS {
        let mut injector = InjectorPP::new();
        injector
            .leak_jit_memory()
            .when_called(injectorpp::func!(foo))
            .will_execute_raw(injectorpp::closure!(
                || {
                    print!("temp\n");
                },
                fn()
            ));
    }

    // If we get here without a crash, the test passes
}
