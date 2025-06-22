use injectorpp::interface::injector::*;
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
        .when_called(injectorpp::func!(Path::exists, fn(&Path) -> bool))
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
        .when_called(injectorpp::func!(
            std::fs::read,
            fn(&'static str) -> std::io::Result<Vec<u8>>
        ))
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
        .when_called(injectorpp::func!(func_no_return, fn()))
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
        .when_called(injectorpp::func!(func_no_return, fn()))
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

    assert_eq!(message_str, "Fake function called more times than expected");
}

#[test]
#[should_panic(
    expected = "Fake function was expected to be called 3 time(s), but it is actually called 2 time(s)"
)]
fn test_will_execute_when_fake_no_return_function_under_called_should_panic() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(func_no_return, fn()))
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
            complex_generic_single_type_always_fail_func,
            fn(&'static str) -> std::io::Result<()>
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
                complex_generic_single_type_always_fail_func,
                fn(&'static str) -> std::io::Result<()>
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
            complex_generic_multiple_types_func,
            fn(&'static str, bool, i32) -> String
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
            single_reference_param_func,
            fn(&mut i32) -> bool
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
            single_reference_param_no_return_func,
            fn(&mut i32) -> ()
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
            multiple_reference_params_func,
            fn(&mut i32, &mut bool) -> bool
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
            multiple_reference_params_no_return_func,
            fn(&mut i32, &mut bool) -> ()
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
        .when_called(injectorpp::func!(Path::is_dir, fn(&Path) -> bool))
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
        .when_called(injectorpp::func!(Foo::bar, fn(&Foo) -> i32))
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
        .when_called(injectorpp::func!(Foo::add, fn(&Foo, i32) -> i32))
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
            Foo::add_no_return,
            fn(&Foo, i32, &mut i32) -> ()
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
                Foo::add_no_return,
                fn(&Foo, i32, &mut i32) -> ()
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
