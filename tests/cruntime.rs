use std::os::raw::{c_char, c_long};

extern "C" {
    fn getenv(name: *const c_char) -> *mut c_char;
    fn time(tloc: *mut c_long) -> c_long;
}

use injectorpp::interface::injector::*;
use std::ffi::{CStr, CString};

#[test]
fn test_fake_getenv_returns_custom_pointer() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            getenv,
            unsafe extern "C" fn(*const c_char) -> *mut c_char
        ))
        .will_execute(injectorpp::fake!(
            func_type: unsafe extern "C" fn(_name: *const c_char) -> *mut c_char,
            returns: CString::new("VALUE").unwrap().into_raw()
        ));

    let name = CString::new("ANY").unwrap();
    let result = unsafe { getenv(name.as_ptr()) };
    let s = unsafe { CStr::from_ptr(result).to_str().unwrap() };
    assert_eq!(s, "VALUE");
}

#[test]
fn test_fake_getenv_when_key_matches() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            getenv,
            unsafe extern "C" fn(*const c_char) -> *mut c_char
        ))
        .will_execute(injectorpp::fake!(
            func_type: unsafe extern "C" fn(name: *const c_char) -> *mut c_char,
            when: unsafe { CStr::from_ptr(name).to_str().unwrap() } == "USER",
            returns: CString::new("USER").unwrap().into_raw()
        ));

    let name_user = CString::new("USER").unwrap();
    let result_user = unsafe { getenv(name_user.as_ptr()) };
    let s2 = unsafe { CStr::from_ptr(result_user).to_str().unwrap() };
    assert_eq!(s2, "USER");
}

#[test]
fn test_fake_getenv_limited_times() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            getenv,
            unsafe extern "C" fn(*const c_char) -> *mut c_char
        ))
        .will_execute(injectorpp::fake!(
            func_type: unsafe extern "C" fn(_name: *const c_char) -> *mut c_char,
            returns: CString::new("LIMIT").unwrap().into_raw(),
            times: 2
        ));

    let name = CString::new("ANY").unwrap();
    unsafe {
        let res1 = getenv(name.as_ptr());
        let s1 = CStr::from_ptr(res1).to_str().unwrap();
        assert_eq!(s1, "LIMIT");

        let res2 = getenv(name.as_ptr());
        let s2 = CStr::from_ptr(res2).to_str().unwrap();
        assert_eq!(s2, "LIMIT");
    }
}

#[test]
fn test_fake_time_assigns_tloc_and_returns_custom() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            time,
            unsafe extern "C" fn(*mut c_long) -> c_long
        ))
        .will_execute(injectorpp::fake!(
            func_type: unsafe extern "C" fn(tloc: *mut c_long) -> c_long,
            assign: { if !tloc.is_null() { *tloc = 123 } },
            returns: 456
        ));

    let mut t: c_long = 0;
    let ret = unsafe { time(&mut t) };

    assert_eq!(t, 123);
    assert_eq!(ret, 456);
}

#[test]
fn test_fake_time_when_and_assign_only_on_non_null() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            time,
            unsafe extern "C" fn(*mut c_long) -> c_long
        ))
        .will_execute(injectorpp::fake!(
            func_type: unsafe extern "C" fn(tloc: *mut c_long) -> c_long,
            when: !tloc.is_null(),
            assign: { *tloc = 7 },
            returns: 8
        ));

    // non-null pointer: fake should run
    let mut t1: c_long = 0;
    let ret1 = unsafe { time(&mut t1) };

    assert_eq!(t1, 7);
    assert_eq!(ret1, 8);
}
