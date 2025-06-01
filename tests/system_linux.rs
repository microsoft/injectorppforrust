#![cfg(target_os = "linux")]

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uint};

use injectorpp::interface::injector::*;

extern "C" {
    fn shm_open(name: *const c_char, oflag: c_int, mode: c_uint) -> c_int;
}

#[test]
fn test_fake_shm_open_should_return_fixed_fd() {
    // Fake shm_open to always return file descriptor 32
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(shm_open))
        .will_execute(injectorpp::fake!(
            func_type: fn(_name: *const c_char, _oflag: c_int, _mode: c_uint) -> c_int,
            returns: 32
        ));

    let name = CString::new("/myshm").unwrap();
    let fd = unsafe { shm_open(name.as_ptr(), 0, 0o600) };
    assert_eq!(fd, 32);
}

#[test]
fn test_fake_shm_open_should_return_error_for_specific_name() {
    // Fake shm_open to return -1 (error) if name is "/fail", otherwise 100
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(shm_open))
        .will_execute(injectorpp::fake!(
            func_type: fn(name: *const c_char, _oflag: c_int, _mode: c_uint) -> c_int,
            when: unsafe { std::ffi::CStr::from_ptr(name).to_str().unwrap() } == "/fail",
            returns: -1
        ));

    let fail_name = CString::new("/fail").unwrap();
    let ok_name = CString::new("/ok").unwrap();

    let fd_fail = unsafe { shm_open(fail_name.as_ptr(), 0, 0o600) };
    assert_eq!(fd_fail, -1);

    // The default behavior (not matched by 'when') will panic, so let's add a second fake for the other case:
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(shm_open))
        .will_execute(injectorpp::fake!(
            func_type: fn(name: *const c_char, _oflag: c_int, _mode: c_uint) -> c_int,
            when: unsafe { std::ffi::CStr::from_ptr(name).to_str().unwrap() } == "/ok",
            returns: 100
        ));

    let fd_ok = unsafe { shm_open(ok_name.as_ptr(), 0, 0o600) };
    assert_eq!(fd_ok, 100);
}
