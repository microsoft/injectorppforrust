use std::os::raw::{c_char, c_int, c_void};

use injectorpp::interface::injector::*;
use std::ffi::{CStr, CString};

extern "C" {
    fn getenv(name: *const c_char) -> *mut c_char;
    fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void;
}

#[test]
fn test_fake_getenv_returns_custom_pointer() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            func_info: unsafe extern "C" fn(getenv)(*const c_char) -> *mut c_char
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
            unsafe{} extern "C" fn(getenv)(*const c_char) -> *mut c_char
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
fn test_fake_memset_assign() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(
            unsafe{} extern "C" fn(memset)(*mut c_void, c_int, usize) -> *mut c_void
        ))
        .will_execute(injectorpp::fake!(
            func_type: unsafe extern "C" fn(s: *mut c_void, _c: c_int, _n: usize) -> *mut c_void,
            assign: {
                let p = s as *mut u8;
                if !p.is_null() {
                    *p = 0x5A;
                }
            },
            returns: s
        ));

    // Prepare a 4-byte buffer, initially all zeros
    let mut buf = [0u8; 4];
    let ptr = buf.as_mut_ptr() as *mut c_void;

    let ret = unsafe { memset(ptr, 0, buf.len()) };

    assert_eq!(buf[0], 0x5A);
    assert_eq!(ret, ptr);
}
