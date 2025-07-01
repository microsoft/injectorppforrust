use injectorpp::interface::injector::*;
use std::fs::File;

#[cfg(target_os = "linux")]
use std::os::fd::FromRawFd;

#[cfg(target_os = "windows")]
use std::os::windows::io::FromRawHandle;

unsafe fn create_fake_file_object() -> File {
    // Create a fake file object using a raw file descriptor
    #[cfg(target_os = "linux")]
    unsafe { File::from_raw_fd(0) }

    #[cfg(target_os = "windows")]
    unsafe { std::fs::File::from_raw_handle(std::ptr::null_mut()) }
}

#[test]
fn test_file_open_fake_result() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (File::open)(&'static str) -> std::io::Result<std::fs::File>))
        .will_execute(injectorpp::fake!(
            func_type: fn(_path: &'static str) -> std::io::Result<std::fs::File>,
            returns: Ok(unsafe { create_fake_file_object() })
        ));

    let result = File::open("/filenotexist");

    assert!(result.is_ok());
}