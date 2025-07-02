use injectorpp::interface::injector::*;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

// For Windows socket API
#[cfg(target_os = "windows")]
use std::os::raw::{c_ulong, c_ushort};

// Socket-related constants and types
#[cfg(target_os = "linux")]
type SocketType = c_int;
#[cfg(target_os = "windows")]
type SocketType = usize;

// Linux socket API declarations
#[cfg(target_os = "linux")]
extern "C" {
    fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
    fn connect(socket: c_int, address: *const c_void, len: u32) -> c_int;
    fn send(socket: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize;
    fn recv(socket: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize;
    fn close(fd: c_int) -> c_int;
    fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
}

// Windows socket API declarations
#[cfg(target_os = "windows")]
extern "system" {
    fn socket(af: c_int, ty: c_int, protocol: c_int) -> SocketType;
    fn connect(s: SocketType, name: *const c_void, namelen: c_int) -> c_int;
    fn send(s: SocketType, buf: *const c_char, len: c_int, flags: c_int) -> c_int;
    fn recv(s: SocketType, buf: *mut c_char, len: c_int, flags: c_int) -> c_int;
    fn closesocket(s: SocketType) -> c_int;
}

// Mock HTTPS response for a 200 OK with proper headers
const MOCK_HTTPS_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nServer: nginx/1.18.0\r\nDate: Tue, 01 Jul 2025 12:00:00 GMT\r\nContent-Type: application/json\r\nContent-Length: 85\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Credentials: true\r\n\r\n{\"args\":{},\"headers\":{\"Host\":\"httpbin.org\"},\"origin\":\"127.0.0.1\",\"url\":\"https://httpbin.org/get\"}";

// TLS handshake mock response (simplified)
const MOCK_TLS_HANDSHAKE: &[u8] = &[
    0x16, 0x03, 0x03, 0x00, 0x7a, // TLS Record Header (Handshake, TLS 1.2, Length 122)
    0x02, 0x00, 0x00, 0x76, // Server Hello message
    0x03,
    0x03, // TLS 1.2 version
          // Mock random data and session info would go here
          // For simplicity, we'll just provide enough bytes to make the TLS handshake "work"
];

static mut RESPONSE_STAGE: usize = 0;
static mut SOCKET_COUNT: usize = 0;

#[tokio::test]
async fn test_hyper_client_always_returns_200_windows() {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            RESPONSE_STAGE = 0;
            SOCKET_COUNT = 0;
        }

        let mut injector = InjectorPP::new();

        // Mock socket creation to return incrementing fake socket handles
        injector
            .when_called(injectorpp::func!(
                unsafe{} extern "system" fn (socket)(c_int, c_int, c_int) -> SocketType
            ))
            .will_execute(injectorpp::fake!(
                func_type: unsafe extern "system" fn(_af: c_int, _ty: c_int, _protocol: c_int) -> SocketType,
                assign: { SOCKET_COUNT += 1; },
                returns: (100 + SOCKET_COUNT) as SocketType // Return incrementing fake socket handles
            ));

        // Mock connect to always succeed
        injector
            .when_called(injectorpp::func!(
                unsafe{} extern "system" fn (connect)(SocketType, *const c_void, c_int) -> c_int
            ))
            .will_execute(injectorpp::fake!(
                func_type: unsafe extern "system" fn(_s: SocketType, _name: *const c_void, _namelen: c_int) -> c_int,
                returns: 0 // Success
            ));

        // Mock send to always succeed
        injector
            .when_called(injectorpp::func!(
                unsafe{} extern "system" fn (send)(SocketType, *const c_char, c_int, c_int) -> c_int
            ))
            .will_execute(injectorpp::fake!(
                func_type: unsafe extern "system" fn(_s: SocketType, _buf: *const c_char, len: c_int, _flags: c_int) -> c_int,
                returns: len // Return the length as if all data was sent
            ));

        // Mock recv to return TLS handshake first, then HTTP response
        injector
            .when_called(injectorpp::func!(
                unsafe{} extern "system" fn (recv)(SocketType, *mut c_char, c_int, c_int) -> c_int
            ))
            .will_execute(injectorpp::fake!(
                func_type: unsafe extern "system" fn(_s: SocketType, buf: *mut c_char, len: c_int, _flags: c_int) -> c_int,
                assign: {
                    RESPONSE_STAGE += 1;
                    if RESPONSE_STAGE <= 3 {
                        // First few calls: return TLS handshake data
                        let response_len = std::cmp::min(MOCK_TLS_HANDSHAKE.len(), len as usize);
                        std::ptr::copy_nonoverlapping(
                            MOCK_TLS_HANDSHAKE.as_ptr(),
                            buf as *mut u8,
                            response_len
                        );
                    } else {
                        // Later calls: return HTTP response
                        let response_len = std::cmp::min(MOCK_HTTPS_RESPONSE.len(), len as usize);
                        std::ptr::copy_nonoverlapping(
                            MOCK_HTTPS_RESPONSE.as_ptr(),
                            buf as *mut u8,
                            response_len
                        );
                    }
                },
                returns: {
                    if RESPONSE_STAGE <= 3 {
                        MOCK_TLS_HANDSHAKE.len() as c_int
                    } else {
                        MOCK_HTTPS_RESPONSE.len() as c_int
                    }
                }
            ));

        // Mock closesocket to always succeed
        injector
            .when_called(injectorpp::func!(
                unsafe{} extern "system" fn (closesocket)(SocketType) -> c_int
            ))
            .will_execute(injectorpp::fake!(
                func_type: unsafe extern "system" fn(_s: SocketType) -> c_int,
                returns: 0 // Success
            ));

        // Now test with hyper client
        use http_body_util::Empty;
        use hyper::{Request, Uri};
        use hyper_util::client::legacy::Client;
        use hyper_util::rt::TokioExecutor;
        use hyper_tls::HttpsConnector;

        let https = HttpsConnector::new();
        let client = Client::builder(TokioExecutor::new()).build(https);

        let uri: Uri = "https://httpbin.org/get".parse().unwrap();
        let req = Request::builder()
            .uri(uri)
            .header("User-Agent", "hyper-test/1.0")
            .body(Empty::<hyper::body::Bytes>::new())
            .unwrap();

        let response = client.request(req).await.unwrap();

        // Verify that we got our mocked 200 response
        assert_eq!(response.status(), 200);
        assert_eq!(response.status().as_u16(), 200);

        println!("✅ Hyper client successfully returned 200 OK for HTTPS request!");
    }
}
