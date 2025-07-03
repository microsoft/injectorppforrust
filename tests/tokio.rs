use injectorpp::interface::injector::*;
use std::ffi::c_void as std_c_void;
use std::io;
use std::net::SocketAddr;
use std::os::raw::{c_int, c_void};
use tokio::net::TcpStream;
use windows_sys::Win32::Networking::WinSock::{
    connect, getsockopt, ioctlsocket, socket, WSAIoctl, WSAPoll, AF_INET, IPPROTO_TCP, SOCKADDR, SOCKET, SOCK_STREAM, SO_ERROR, WSAPOLLFD, WSAGetLastError
};
use windows_sys::Win32::System::IO::{CreateIoCompletionPort, GetQueuedCompletionStatusEx, OVERLAPPED_ENTRY};
use windows_sys::Win32::Storage::FileSystem::SetFileCompletionNotificationModes;
use windows_sys::Win32::Foundation::HANDLE;

static mut FAKE_SOCKET_COUNTER: SOCKET = 1000;
static mut FAKE_HANDLE_COUNTER: usize = 2000;

fn fake_socket(_af: i32, _type: i32, _protocol: i32) -> SOCKET {
    unsafe {
        FAKE_SOCKET_COUNTER += 1;
        FAKE_SOCKET_COUNTER
    }
}

fn fake_ioctlsocket(_s: SOCKET, _cmd: i32, _argp: *mut u32) -> i32 {
    return 0; // Success
}

fn fake_connect(_s: SOCKET, _name: *const SOCKADDR, _namelen: i32) -> i32 {
    return 0; // Success
}

fn fake_wsapoll(fdarray: *mut WSAPOLLFD, fds: u32, timeout: i32) -> i32 {
    // Signal that the socket is writable
    if !fdarray.is_null() && fds > 0 {
        unsafe {
            (*fdarray).revents = 0x10; // POLLOUT - writable
        }
    }
    return 1; // One socket ready
}

fn fake_getsockopt(s: SOCKET, level: i32, optname: i32, optval: *mut u8, optlen: *mut i32) -> i32 {
    // If checking for SO_ERROR, return no error
    if level == 0xFFFF && optname == SO_ERROR as i32 {
        if !optval.is_null() && !optlen.is_null() {
            unsafe {
                *(optval as *mut i32) = 0; // No error
                *optlen = 4;
            }
        }
    }
    return 0; // Success
}

fn fake_create_io_completion_port(
    _file_handle: HANDLE,
    _existing_completion_port: HANDLE,
    _completion_key: usize,
    _number_of_concurrent_threads: u32,
) -> HANDLE {
    unsafe {
        FAKE_HANDLE_COUNTER += 1;
        FAKE_HANDLE_COUNTER as HANDLE
    }
}

fn fake_set_file_completion_notification_modes(_file_handle: HANDLE, _flags: u8) -> i32 {
    return 1; // Success (non-zero)
}

fn fake_wsa_ioctl(
    s: SOCKET,
    dwIoControlCode: u32,
    lpvInBuffer: *mut c_void,
    cbInBuffer: u32,
    lpvOutBuffer: *mut c_void,
    cbOutBuffer: u32,
    lpcbBytesReturned: *mut u32,
    lpOverlapped: *mut c_void,
    lpCompletionRoutine: *mut c_void,
) -> i32 {
    // For base socket handle requests, return the same socket handle
    if !lpvOutBuffer.is_null() && cbOutBuffer >= 8 {
        unsafe {
            *(lpvOutBuffer as *mut SOCKET) = s; // Return the same socket as base
            if !lpcbBytesReturned.is_null() {
                *lpcbBytesReturned = 8; // sizeof(SOCKET) on 64-bit
            }
        }
    }
    return 0; // Success
}

fn fake_wsa_get_last_error() -> i32 {
    return 0; // No error
}

fn fake_get_queued_completion_status_ex(
    completion_port: HANDLE,
    completion_port_entries: *mut OVERLAPPED_ENTRY,
    ul_count: u32,
    ul_num_entries_removed: *mut u32,
    dw_milliseconds: u32,
    f_alertable: i32,
) -> i32 {
    // Simulate that we have completion events immediately available
    if !completion_port_entries.is_null() && ul_count > 0 && !ul_num_entries_removed.is_null() {
        unsafe {
            // Create a fake completion entry that indicates the socket is ready
            let entry = &mut *completion_port_entries;
            entry.lpCompletionKey = 1; // Some fake completion key
            entry.lpOverlapped = std::ptr::null_mut(); // Custom event (no overlapped)
            entry.dwNumberOfBytesTransferred = 0;
            entry.Internal = 0;
            
            *ul_num_entries_removed = 1; // One event available
        }
    }
    return 1; // Success (non-zero)
}

#[tokio::test]
async fn test_tcp_connect_mock_comprehensive() {
    let mut injector = InjectorPP::new();

    unsafe {
        // Mock socket creation
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(socket))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_socket));

        // Mock setting socket to non-blocking mode
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(ioctlsocket))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_ioctlsocket));
            
        // Mock connect
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(connect))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_connect));
            
        // Mock polling
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(WSAPoll))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_wsapoll));
            
        // Mock error checking
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(getsockopt))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_getsockopt));

        // Mock IOCP registration
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(CreateIoCompletionPort))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_create_io_completion_port));

        // Mock file completion notification modes
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(SetFileCompletionNotificationModes))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_set_file_completion_notification_modes));

        // Mock WSAIoctl (for base socket handle queries)
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(WSAIoctl))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_wsa_ioctl));

        // Mock WSAGetLastError
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(WSAGetLastError))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_wsa_get_last_error));

        // Mock GetQueuedCompletionStatusEx (the critical one that was causing the hang)
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(GetQueuedCompletionStatusEx))
            .will_execute_raw_unchecked(injectorpp::func_unchecked!(fake_get_queued_completion_status_ex));
    }

    let result = TcpStream::connect("192.168.254.254:9999").await;
    let r = result.is_ok();
    if (result.is_err()) {
        eprintln!("Connection failed: {:?}", result.err());
    }

    assert!(r, "Connection should succeed due to comprehensive mocking");
}