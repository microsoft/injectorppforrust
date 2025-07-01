use injectorpp::interface::injector::*;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Result;
use std::io::Write;

#[cfg(target_os = "linux")]
use std::os::fd::FromRawFd;

#[cfg(target_os = "windows")]
use std::os::windows::io::FromRawHandle;

#[cfg(target_os = "windows")]
extern "system" {
    fn CreatePipe(
        hReadPipe: *mut *mut std::ffi::c_void,
        hWritePipe: *mut *mut std::ffi::c_void,
        lpPipeAttributes: *const std::ffi::c_void,
        nSize: u32,
    ) -> i32;

    fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
}

unsafe fn create_fake_file_object() -> File {
    // Create a fake file object using a raw file descriptor
    #[cfg(target_os = "linux")]
    unsafe {
        // Create a pipe, use the read end, and immediately close the write end
        let mut fds = [0; 2];
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            panic!("Failed to create pipe for fake file");
        }

        // Close the write end immediately
        libc::close(fds[1]);

        // Return the read end as a File
        File::from_raw_fd(fds[0])
    }

    #[cfg(target_os = "windows")]
    unsafe {
        let mut read_handle = std::ptr::null_mut();
        let mut write_handle = std::ptr::null_mut();

        CreatePipe(&mut read_handle, &mut write_handle, std::ptr::null_mut(), 0);
        CloseHandle(write_handle);

        std::fs::File::from_raw_handle(read_handle)
    }
}

#[test]
fn test_file_open_fake_result() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(
            injectorpp::func!(fn (File::open)(&'static str) -> std::io::Result<std::fs::File>),
        )
        .will_execute(injectorpp::fake!(
            func_type: fn(_path: &'static str) -> std::io::Result<std::fs::File>,
            returns: Ok(unsafe { create_fake_file_object() })
        ));

    let result = File::open("/filenotexist");

    assert!(result.is_ok());
}

#[test]
fn test_read_line_fake_result() {
    let mut injector = InjectorPP::new();

    injector
        .when_called(
            injectorpp::func!(fn (File::open)(&'static str) -> std::io::Result<std::fs::File>),
        )
        .will_execute(injectorpp::fake!(
            func_type: fn(_path: &'static str) -> std::io::Result<std::fs::File>,
            returns: Ok(unsafe { create_fake_file_object() })
        ));

    injector
        .when_called(injectorpp::func!(fn (BufReader::<File>::read_line)(&mut BufReader<File>, &mut String) -> Result<usize>))
        .will_execute(injectorpp::fake!(
            func_type: fn(_reader: &mut BufReader<File>, line: &mut String) -> Result<usize>,
            assign: { *line = "Fake line content".to_string() },
            returns: Ok(line.len())
        ));

    let file = File::open("/not/exist/path").unwrap();
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    let result = reader.read_line(&mut line);

    assert!(result.is_ok());
    assert_eq!(line, "Fake line content");
    assert_eq!(result.unwrap(), 17);
}

#[test]
fn test_write_all_fake_result() {
    let mut injector = InjectorPP::new();

    injector
        .when_called(
            injectorpp::func!(fn (File::open)(&'static str) -> std::io::Result<std::fs::File>),
        )
        .will_execute(injectorpp::fake!(
            func_type: fn(_path: &'static str) -> std::io::Result<std::fs::File>,
            returns: Ok(unsafe { create_fake_file_object() })
        ));

    injector
        .when_called(injectorpp::func!(fn (File::write_all)(&mut File, &[u8]) -> Result<()>))
        .will_execute(injectorpp::fake!(
            func_type: fn(_file: &mut File, _buf: &[u8]) -> Result<()>,
            returns: Ok(())
        ));

    let file = File::open("/not/exist/path").unwrap();
    let mut file = file;
    let data = b"Hello, world!";

    let result = file.write_all(data);

    assert!(result.is_ok());
}
