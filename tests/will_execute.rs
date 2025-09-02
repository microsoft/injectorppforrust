use injectorpp::interface::injector::*;
use std::path::MAIN_SEPARATOR;
use std::fmt::Display;
use std::path::Path;

pub fn fake_path_exists() -> bool {
    println!("fake_path_exists executed.");
    true
}

pub fn func_no_return() {
    panic!();
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

fn single_reference_param_no_return_func(a: &mut i32) {
    *a = 1;
}

fn multiple_reference_params_no_return_func(a: &mut i32, b: &mut bool) {
    *a = 1;
    *b = false;
}

fn single_reference_param_func(a: &mut i32) -> bool {
    *a = 1;

    return false;
}

fn multiple_reference_params_func(a: &mut i32, b: &mut bool) -> bool {
    *a = 1;
    *b = false;

    return false;
}

pub unsafe fn unsafe_non_unit(a: i32) -> i32 {
    a * 10
}

pub unsafe fn unsafe_unit(x: &mut i32) {
    *x += 2;
}

pub struct Foo {
    value: i32,
}

impl Foo {
    pub fn bar(&self) -> i32 {
        println!("The value is {}", self.value);

        self.value
    }

    pub fn add(&self, value: i32) -> i32 {
        self.value + value
    }

    pub fn add_no_return(&self, value: i32, output: &mut i32) {
        *output = self.value + value;
    }

    pub fn new(v: i32) -> Self {
        Foo { value: v }
    }
}

#[test]
fn test_will_execute_when_fake_file_dependency_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (Path::exists)(&Path) -> bool))
        .will_execute(injectorpp::fake!(
            func_type: fn(_path: &Path) -> bool,
            returns: true
        ));

    let test_path = "/path/that/does/not/exist";
    let result = Path::new(test_path).exists();

    assert_eq!(result, true);
}

#[test]
fn test_will_execute_when_fake_std_fs_read_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(
            injectorpp::func!(fn (std::fs::read)(&'static str) -> std::io::Result<Vec<u8>>),
        )
        .will_execute(injectorpp::fake!(
            func_type: fn(_path: &str) -> std::io::Result<Vec<u8>>,
            returns: Ok(vec![1, 2, 3])
        ));

    let data = std::fs::read("fake.txt").unwrap();
    assert_eq!(data, vec![1, 2, 3]);
}

#[test]
fn test_will_execute_when_fake_no_return_function_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (func_no_return)()))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> (),
            times: 1
        ));

    let result = std::panic::catch_unwind(|| {
        func_no_return();
    });

    assert!(result.is_ok());
}

#[test]
#[should_panic(
    expected = "Fake function was expected to be called 2 time(s), but it is actually called 3 time(s)"
)]
fn test_will_execute_when_fake_no_return_function_over_called_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (func_no_return)()))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> (),
            times: 2
        ));

    func_no_return();
    func_no_return();

    let result = std::panic::catch_unwind(|| {
        func_no_return();
    });

    assert!(result.is_err());

    let message = result.unwrap_err();
    let message_str = message
        .downcast_ref::<&str>()
        .map(|s| *s)
        .or_else(|| message.downcast_ref::<String>().map(|s| s.as_str()))
        .unwrap();

    assert_eq!(
        message_str,
        format!("Fake function defined at tests{MAIN_SEPARATOR}will_execute.rs:143:23 called more times than expected")
    );
}

#[test]
#[should_panic(
    expected = "Fake function was expected to be called 3 time(s), but it is actually called 2 time(s)"
)]
fn test_will_execute_when_fake_no_return_function_under_called_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (func_no_return)()))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> (),
            times: 3
        ));

    func_no_return();
    func_no_return();
}

#[test]
fn test_will_execute_when_fake_generic_function_single_type_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (complex_generic_single_type_always_fail_func)(&'static str) -> std::io::Result<()>
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn(path: &str) -> std::io::Result<()>,
            when: path == "/not/exist/path",
            returns: Ok(()),
            times: 1
        ));

    let actual_result = complex_generic_single_type_always_fail_func("/not/exist/path");

    assert!(actual_result.is_ok());
}

#[test]
fn test_will_execute_when_fake_generic_function_single_type_can_recover() {
    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(
                fn (complex_generic_single_type_always_fail_func)(&'static str) -> std::io::Result<()>
            ))
            .will_execute(injectorpp::fake!(
                func_type: fn(path: &str) -> std::io::Result<()>,
                when: path == "/not/exist/path",
                returns: Ok(()),
                times: 1
            ));

        let actual_result = complex_generic_single_type_always_fail_func("/not/exist/path");

        assert!(actual_result.is_ok());
    }

    let actual_result = complex_generic_single_type_always_fail_func("/not/exist/path");

    assert!(actual_result.is_err());
}

#[test]
fn test_will_execute_when_fake_generic_function_multiple_types_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (complex_generic_multiple_types_func)(&'static str, bool, i32) -> String
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn(a: &str, b: bool, c: i32) -> String,
            when: a == "abc" && b == true && c == 123,
            returns: "Fake value".to_string(),
            times: 1
        ));

    let actual_result = complex_generic_multiple_types_func("abc", true, 123);

    // This call should not be counted as the types are different from the fake_closure.
    complex_generic_multiple_types_func(1, 2, 3);

    assert_eq!(actual_result, "Fake value".to_string());
}

#[test]
fn test_will_execute_when_fake_single_reference_param_function_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (single_reference_param_func)(&mut i32) -> bool
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn(a: &mut i32) -> bool,
            assign: { *a = 6 },
            returns: true
        ));

    let mut value = 0;

    let result = single_reference_param_func(&mut value);

    assert_eq!(value, 6);
    assert_eq!(result, true);
}

#[test]
fn test_will_execute_when_fake_single_reference_param_no_return_function_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (single_reference_param_no_return_func)(&mut i32) -> ()
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn(a: &mut i32) -> (),
            assign: { *a = 6 }
        ));

    let mut result = 0;

    single_reference_param_no_return_func(&mut result);

    assert_eq!(result, 6);
}

#[test]
fn test_will_execute_when_fake_multiple_reference_param_function_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (multiple_reference_params_func)(&mut i32, &mut bool) -> bool
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn(a: &mut i32, b: &mut bool) -> bool,
            assign: { *a = 6; *b = true },
            returns: true,
            times: 1
        ));

    let mut value1 = 0;
    let mut value2 = false;

    let result = multiple_reference_params_func(&mut value1, &mut value2);

    assert_eq!(value1, 6);
    assert_eq!(value2, true);
    assert_eq!(result, true);
}

#[test]
fn test_will_execute_when_fake_multiple_reference_param_no_return_function_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            fn (multiple_reference_params_no_return_func)(&mut i32, &mut bool) -> ()
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn(a: &mut i32, b: &mut bool) -> (),
            assign: { *a = 6; *b = true },
            times: 1
        ));

    let mut value1 = 0;
    let mut value2 = false;

    multiple_reference_params_no_return_func(&mut value1, &mut value2);

    assert_eq!(value1, 6);
    assert_eq!(value2, true);
}

#[test]
fn test_will_execute_when_fake_file_dependency_should_success_times() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (Path::is_dir)(&Path) -> bool))
        .will_execute(injectorpp::fake!(
            func_type: fn(_path: &Path) -> bool,
            returns: true,
            times: 2
        ));

    let path = Path::new("/path/that/does/not/exist");
    assert!(path.is_dir());
    assert!(path.is_dir());
}

#[test]
fn test_will_execute_when_fake_method_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (Foo::bar)(&Foo) -> i32))
        .will_execute(injectorpp::fake!(
            func_type: fn(_f: &Foo) -> i32,
            returns: 1
        ));

    let foo = Foo::new(6);
    let result = foo.bar();

    assert_eq!(result, 1);
}

#[test]
fn test_will_execute_when_fake_method_with_parameter_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (Foo::add)(&Foo, i32) -> i32))
        .will_execute(injectorpp::fake!(
            func_type: fn(f: &Foo, value: i32) -> i32,
            when: f.value > 0,
            returns: f.value * 2 + value * 2
        ));

    let foo = Foo::new(6);
    let result = foo.add(3);

    assert_eq!(result, 18);
}

#[test]
fn test_will_execute_when_fake_method_with_output_parameter_no_return_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            func_info: fn (Foo::add_no_return)(&Foo, i32, &mut i32) -> ()
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn(f: &Foo, value: i32, output: &mut i32) -> (),
            when: f.value > 0,
            assign: { *output = f.value * 2 + value * 2 }
        ));

    let foo = Foo::new(6);
    let mut result = 0;
    foo.add_no_return(3, &mut result);

    assert_eq!(result, 18);
}

#[test]
fn test_will_execute_when_fake_method_can_recover() {
    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(
                fn (Foo::add_no_return)(&Foo, i32, &mut i32) -> ()
            ))
            .will_execute(injectorpp::fake!(
                func_type: fn(f: &Foo, value: i32, output: &mut i32) -> (),
                when: f.value > 0,
                assign: { *output = f.value * 2 + value * 2 }
            ));

        let foo = Foo::new(6);
        let mut result = 0;
        foo.add_no_return(3, &mut result);

        assert_eq!(result, 18);
    }

    let foo = Foo::new(6);
    let mut result = 0;
    foo.add_no_return(3, &mut result);

    assert_eq!(result, 9);
}

#[test]
fn test_will_execute_fake_unsafe_non_unit_returns_only_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(unsafe{} fn (unsafe_non_unit)(i32) -> i32))
        .will_execute(injectorpp::fake!(
            func_type: unsafe fn(val: i32) -> i32,
            returns: val + 1
        ));

    let result = unsafe { unsafe_non_unit(5) };

    assert_eq!(result, 6);
}

#[test]
#[should_panic(
    expected = "Fake function was expected to be called 2 time(s), but it is actually called 3 time(s)"
)]
fn test_will_execute_fake_unsafe_non_unit_returns_and_times_over_called_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(unsafe{} fn (unsafe_non_unit)(i32) -> i32))
        .will_execute(injectorpp::fake!(
            func_type: unsafe fn(val: i32) -> i32,
            returns: val + 2,
            times: 2
        ));

    unsafe {
        assert_eq!(unsafe_non_unit(1), 3);
        assert_eq!(unsafe_non_unit(2), 4);
    }

    let result = std::panic::catch_unwind(|| unsafe { unsafe_non_unit(3) });
    assert!(result.is_err());
}

#[test]
fn test_will_execute_fake_unsafe_unit_without_times_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(unsafe{} fn (unsafe_unit)(&mut i32) -> ()))
        .will_execute(injectorpp::fake!(
            func_type: unsafe fn(_x: &mut i32) -> ()
        ));

    let mut val = 10;

    unsafe { unsafe_unit(&mut val) };

    // fake does nothing, original behavior skipped
    assert_eq!(val, 10);
}

#[test]
#[should_panic(
    expected = "Fake function was expected to be called 1 time(s), but it is actually called 2 time(s)"
)]
fn test_will_execute_fake_unsafe_unit_with_times_over_called_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(unsafe{} fn (unsafe_unit)(&mut i32) -> ()))
        .will_execute(injectorpp::fake!(
            func_type: unsafe fn(_x: &mut i32) -> (),
            times: 1
        ));

    let mut val1 = 5;
    unsafe { unsafe_unit(&mut val1) };

    assert_eq!(val1, 5);

    let result = std::panic::catch_unwind(|| unsafe {
        let mut val2 = 5;
        unsafe_unit(&mut val2)
    });
    assert!(result.is_err());
}

#[test]
fn test_will_execute_fake_unsafe_unit_with_assign_only_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(unsafe{} fn (unsafe_unit)(&mut i32) -> ()))
        .will_execute(injectorpp::fake!(
            func_type: unsafe fn(x: &mut i32) -> (),
            assign: { *x += 5 }
        ));

    let mut val = 0;

    unsafe { unsafe_unit(&mut val) };
    assert_eq!(val, 5);
}

#[test]
fn test_will_execute_fake_unsafe_unit_with_assign_and_times_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(func_info: unsafe fn (unsafe_unit)(&mut i32) -> ()))
        .will_execute(injectorpp::fake!(
            func_type: unsafe fn(x: &mut i32) -> (),
            assign: { *x += 2 },
            times: 2
        ));

    let mut val = 1;
    unsafe { unsafe_unit(&mut val) };
    unsafe { unsafe_unit(&mut val) };

    assert_eq!(val, 5);
}

#[test]
#[should_panic(expected = "Fake function was expected to be called 2 time(s)")]
fn test_will_execute_fake_unsafe_unit_assign_and_times_over_called_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(unsafe{} fn (unsafe_unit)(&mut i32) -> ()))
        .will_execute(injectorpp::fake!(
            func_type: unsafe fn(x: &mut i32) -> (),
            assign: { *x += 3 },
            times: 2
        ));

    let mut val = 0;
    unsafe { unsafe_unit(&mut val) };
    unsafe { unsafe_unit(&mut val) };

    let result = std::panic::catch_unwind(|| unsafe {
        let mut val = 0;
        unsafe_unit(&mut val)
    });
    assert!(result.is_err());
}
