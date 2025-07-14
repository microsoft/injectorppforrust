use injectorpp::interface::injector::*;
use std::thread;

#[inline(never)]
pub fn foo() -> i32 {
    6
}

#[test]
fn test_multi_thread_function_call() {
    let handle = thread::spawn(move || {
        for _ in 0..1000 {
            InjectorPP::prevent();

            assert_eq!(foo(), 6);
        }
    });

    for _ in 0..10 {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn (foo)() -> i32))
            .will_execute_raw(injectorpp::closure!(|| { 9 }, fn() -> i32));

        assert_eq!(foo(), 9);
    }

    handle.join().unwrap();
}
