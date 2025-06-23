use injectorpp::interface::injector::*;
use std::fmt::Display;
use std::path::Path;
use std::sync::atomic::*;

static CALL_COUNT_FUNC: AtomicUsize = AtomicUsize::new(0);

pub fn fake_path_exists(_path: &Path) -> bool {
    println!("fake_path_exists executed.");
    true
}

pub fn func_no_return() {
    panic!();
}

pub fn fake_func_no_return() {
    CALL_COUNT_FUNC.fetch_add(1, Ordering::SeqCst);
}

pub fn complex_generic_single_type_always_fail_func<P: AsRef<Path>>(
    _path: P,
) -> Result<(), std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Always returns error",
    ))
}

fn complex_generic_multiple_types_func<A: Display, B: Display, C: Display>(
    _a: A,
    _b: B,
    _c: C,
) -> String {
    return "Original value".to_string();
}

#[test]
fn test_will_execute_raw_when_fake_file_dependency_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (Path::exists)(&Path) -> bool))
        .will_execute_raw(injectorpp::func!(fn (fake_path_exists)(&Path) -> bool));

    let test_path = "/path/that/does/not/exist";
    let result = Path::new(test_path).exists();

    assert_eq!(result, true);
}

#[test]
fn test_will_execute_raw_when_fake_no_return_function_should_success() {
    CALL_COUNT_FUNC.store(0, Ordering::SeqCst);

    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (func_no_return)()))
        .will_execute_raw(injectorpp::func!(fn (fake_func_no_return)()));

    func_no_return();

    let count = CALL_COUNT_FUNC.load(Ordering::SeqCst);

    assert_eq!(count, 1);
}

#[test]
fn test_will_execute_raw_when_fake_no_return_function_use_closure_should_success() {
    static CALL_COUNT_CLOSURE: AtomicU32 = AtomicU32::new(0);

    let fake_closure = || {
        CALL_COUNT_CLOSURE.fetch_add(1, Ordering::SeqCst);
    };

    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (func_no_return)()))
        .will_execute_raw(injectorpp::closure!(fake_closure, fn()));

    func_no_return();

    assert_eq!(CALL_COUNT_CLOSURE.load(Ordering::SeqCst), 1);
}

#[test]
fn test_will_execute_raw_when_fake_generic_function_single_type_should_success() {
    static CALL_COUNT_CLOSURE: AtomicU32 = AtomicU32::new(0);
    let fake_closure = |_path: &str| -> std::io::Result<()> {
        CALL_COUNT_CLOSURE.fetch_add(1, Ordering::SeqCst);

        Ok(())
    };

    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (complex_generic_single_type_always_fail_func)(&'static str) -> std::io::Result<()>
        ))
        .will_execute_raw(injectorpp::closure!(
            fake_closure,
            fn(&str) -> std::io::Result<()>
        ));

    let actual_result = complex_generic_single_type_always_fail_func("/not/exist/path");

    assert_eq!(CALL_COUNT_CLOSURE.load(Ordering::SeqCst), 1);
    assert!(actual_result.is_ok());
}

#[test]
fn test_will_execute_raw_when_fake_generic_function_multiple_types_should_success() {
    static CALL_COUNT_CLOSURE: AtomicU32 = AtomicU32::new(0);
    let fake_closure = |_a: &str, _b: bool, _c: i32| -> String {
        CALL_COUNT_CLOSURE.fetch_add(1, Ordering::SeqCst);

        "Fake value".to_string()
    };

    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (complex_generic_multiple_types_func)(&'static str, bool, i32) -> String
        ))
        .will_execute_raw(injectorpp::closure!(
            fake_closure,
            fn(&str, bool, i32) -> String
        ));

    let actual_result = complex_generic_multiple_types_func("abc", true, 123);

    // This call should not be counted as the types are different from the fake_closure.
    // CALL_COUNT_CLOSURE should not increase.
    complex_generic_multiple_types_func(1, 2, 3);

    assert_eq!(CALL_COUNT_CLOSURE.load(Ordering::SeqCst), 1);
    assert_eq!(actual_result, "Fake value".to_string());
}

#[test]
fn test_will_execute_raw_when_fake_generic_function_multiple_types_with_different_conditins_should_success(
) {
    static CALL_COUNT_CONDITION_ONE_CLOSURE: AtomicU32 = AtomicU32::new(0);
    static CALL_COUNT_CONDITION_TWO_CLOSURE: AtomicU32 = AtomicU32::new(0);
    static CALL_COUNT_CONDITION_THREE_CLOSURE: AtomicU32 = AtomicU32::new(0);

    let fake_closure = |a: &str, b: bool, c: i32| -> String {
        if b == true && c > 1 {
            CALL_COUNT_CONDITION_ONE_CLOSURE.fetch_add(1, Ordering::SeqCst);

            return "Called with condition 1".to_string();
        }

        if a == "cond2" && b == false {
            CALL_COUNT_CONDITION_TWO_CLOSURE.fetch_add(1, Ordering::SeqCst);

            return "Called with condition 2".to_string();
        }

        CALL_COUNT_CONDITION_THREE_CLOSURE.fetch_add(1, Ordering::SeqCst);

        "Called with condition 3".to_string()
    };

    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (complex_generic_multiple_types_func)(&'static str, bool, i32) -> String
        ))
        .will_execute_raw(injectorpp::closure!(
            fake_closure,
            fn(&str, bool, i32) -> String
        ));

    // Call the function with condition 1 twice.
    let actual_result = complex_generic_multiple_types_func("abc", true, 123);
    assert_eq!(actual_result, "Called with condition 1".to_string());

    complex_generic_multiple_types_func("abc", true, 2);

    // Call the function with condition 2 once.
    let actual_result = complex_generic_multiple_types_func("cond2", false, 123);
    assert_eq!(actual_result, "Called with condition 2".to_string());

    // Call the function with condition 3 twice.
    let actual_result = complex_generic_multiple_types_func("abc", false, 123);
    assert_eq!(actual_result, "Called with condition 3".to_string());

    complex_generic_multiple_types_func("abc", false, 123);

    assert_eq!(CALL_COUNT_CONDITION_ONE_CLOSURE.load(Ordering::SeqCst), 2);
    assert_eq!(CALL_COUNT_CONDITION_TWO_CLOSURE.load(Ordering::SeqCst), 1);
    assert_eq!(CALL_COUNT_CONDITION_THREE_CLOSURE.load(Ordering::SeqCst), 2);
}
