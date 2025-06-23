use std::os::raw::c_char;

extern "C" {
    fn getenv(name: *const c_char) -> *mut c_char;
}

use std::ffi::{CString, CStr};
use injectorpp::interface::injector::*;

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