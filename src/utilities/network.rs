//! HTTP Mock utilities for injectorpp
//!
//! This module provides easy-to-use abstractions for mocking HTTP responses
//! at the socket level, allowing any HTTP client (like hyper) to receive
//! predefined responses without making actual network calls.

use crate::interface::injector::*;
use std::cell::Cell;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};

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

// Global counters for socket simulation
static RESPONSE_STAGE: AtomicUsize = AtomicUsize::new(0);
static SOCKET_COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static MOCK_RESPONSE_DATA_LEN: Cell<usize> = const { Cell::new(0) };
    static MOCK_RESPONSE_DATA_PTR: Cell<*const u8> = const { Cell::new(std::ptr::null()) };
    static MOCK_DELAY_CALLS: Cell<usize> = const { Cell::new(3) };
}

/// HTTP status codes commonly used in testing
#[derive(Debug, Clone, Copy)]
pub enum HttpStatus {
    Ok = 200,
    Created = 201,
    NoContent = 204,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    InternalServerError = 500,
    BadGateway = 502,
    ServiceUnavailable = 503,
}

impl HttpStatus {
    pub fn as_u16(self) -> u16 {
        self as u16
    }

    pub fn reason_phrase(self) -> &'static str {
        match self {
            HttpStatus::Ok => "OK",
            HttpStatus::Created => "Created",
            HttpStatus::NoContent => "No Content",
            HttpStatus::BadRequest => "Bad Request",
            HttpStatus::Unauthorized => "Unauthorized",
            HttpStatus::Forbidden => "Forbidden",
            HttpStatus::NotFound => "Not Found",
            HttpStatus::InternalServerError => "Internal Server Error",
            HttpStatus::BadGateway => "Bad Gateway",
            HttpStatus::ServiceUnavailable => "Service Unavailable",
        }
    }
}

/// Configuration for HTTP response mocking
#[derive(Debug, Clone)]
pub struct HttpMockConfig {
    pub status: HttpStatus,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub delay_calls: usize, // Number of socket calls before returning the response
}

impl Default for HttpMockConfig {
    fn default() -> Self {
        Self {
            status: HttpStatus::Ok,
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Server".to_string(), "nginx/1.18.0".to_string()),
                (
                    "Date".to_string(),
                    "Tue, 01 Jul 2025 12:00:00 GMT".to_string(),
                ),
                ("Connection".to_string(), "close".to_string()),
            ],
            body: r#"{"status": "success", "message": "Mocked response"}"#.to_string(),
            delay_calls: 3, // Default delay for TLS handshake simulation
        }
    }
}

impl HttpMockConfig {
    /// Create a new HTTP mock configuration with 200 OK status
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the HTTP status code
    pub fn with_status(mut self, status: HttpStatus) -> Self {
        self.status = status;
        self
    }

    /// Add a custom header
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Set the response body
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    /// Set JSON response body
    pub fn with_json_body(mut self, json: impl Into<String>) -> Self {
        self.body = json.into();
        // Ensure Content-Type is set to JSON
        self.headers
            .retain(|(name, _)| name.to_lowercase() != "content-type");
        self.headers
            .push(("Content-Type".to_string(), "application/json".to_string()));
        self
    }

    /// Set the number of socket calls to delay before returning the response
    /// This is useful for simulating TLS handshakes
    pub fn with_delay_calls(mut self, delay: usize) -> Self {
        self.delay_calls = delay;
        self
    }

    /// Generate the complete HTTP response as bytes
    pub fn to_response_bytes(&self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} {}\r\n",
            self.status.as_u16(),
            self.status.reason_phrase()
        );

        // Add Content-Length header
        let body_len = self.body.len();
        let mut has_content_length = false;
        for (name, _) in &self.headers {
            if name.to_lowercase() == "content-length" {
                has_content_length = true;
                break;
            }
        }
        if !has_content_length {
            response.push_str(&format!("Content-Length: {}\r\n", body_len));
        }

        // Add all headers
        for (name, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", name, value));
        }

        // Add header/body separator and body
        response.push_str("\r\n");
        response.push_str(&self.body);

        response.into_bytes()
    }
}

/// Simple TLS handshake simulation data
const MOCK_TLS_HANDSHAKE: &[u8] = &[
    0x16, 0x03, 0x03, 0x00, 0x7a, // TLS Record Header (Handshake, TLS 1.2, Length 122)
    0x02, 0x00, 0x00, 0x76, // Server Hello message
    0x03, 0x03, // TLS 1.2 version
    // Additional mock data to make it look like a valid handshake
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
];

/// HTTP Mock Manager - handles setting up socket-level mocks
pub struct HttpMocker {
    config: HttpMockConfig,
    response_bytes: Vec<u8>,
}

impl HttpMocker {
    /// Create a new HTTP mocker with the given configuration
    pub fn new(config: HttpMockConfig) -> Self {
        let response_bytes = config.to_response_bytes();
        Self {
            config,
            response_bytes,
        }
    }

    /// Create a simple 200 OK mocker
    pub fn ok() -> Self {
        Self::new(HttpMockConfig::new())
    }

    /// Create a mocker that returns the specified status
    pub fn with_status(status: HttpStatus) -> Self {
        Self::new(HttpMockConfig::new().with_status(status))
    }

    /// Create a mocker with JSON response
    pub fn with_json(json: impl Into<String>) -> Self {
        Self::new(HttpMockConfig::new().with_json_body(json))
    }

    /// Create a mocker for error responses
    pub fn error(status: HttpStatus, message: impl Into<String>) -> Self {
        let error_body = format!(
            r#"{{"error": "{}", "status": {}}}"#,
            message.into(),
            status.as_u16()
        );
        Self::new(
            HttpMockConfig::new()
                .with_status(status)
                .with_json_body(error_body),
        )
    }

    /// Install the socket mocks for the current platform
    pub fn install(&self, injector: &mut InjectorPP) {
        // Reset global counters
        RESPONSE_STAGE.store(0, Ordering::SeqCst);
        SOCKET_COUNT.store(0, Ordering::SeqCst);

        #[cfg(target_os = "windows")]
        self.install_windows_mocks(injector);

        #[cfg(target_os = "linux")]
        self.install_linux_mocks(injector);
    }

    #[cfg(target_os = "windows")]
    fn install_windows_mocks(&self, injector: &mut InjectorPP) {
        let response_bytes = self.response_bytes.clone();

        MOCK_DELAY_CALLS.with(|c| c.set(self.config.delay_calls));

        // Mock socket creation
        injector
            .when_called(crate::func!(
                unsafe{} extern "system" fn (socket)(c_int, c_int, c_int) -> SocketType
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "system" fn(_af: c_int, _ty: c_int, _protocol: c_int) -> SocketType,
                assign: {
                    SOCKET_COUNT.fetch_add(1, Ordering::SeqCst);
                },
                returns: {
                    let count = SOCKET_COUNT.load(Ordering::SeqCst);
                    (100 + count) as SocketType
                }
            ));

        // Mock connect to always succeed
        injector
            .when_called(crate::func!(
                unsafe{} extern "system" fn (connect)(SocketType, *const c_void, c_int) -> c_int
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "system" fn(_s: SocketType, _name: *const c_void, _namelen: c_int) -> c_int,
                returns: 0
            ));

        // Mock send to always succeed
        injector
            .when_called(crate::func!(
                unsafe{} extern "system" fn (send)(SocketType, *const c_char, c_int, c_int) -> c_int
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "system" fn(_s: SocketType, _buf: *const c_char, len: c_int, _flags: c_int) -> c_int,
                returns: len
            ));

        // Mock recv to return appropriate data
        let response_clone = response_bytes.clone();
        let ptr = response_clone.as_ptr();

        MOCK_RESPONSE_DATA_LEN.with(|c| c.set(response_clone.len()));
        MOCK_RESPONSE_DATA_PTR.with(|c| c.set(ptr));

        injector
            .when_called(crate::func!(
                unsafe{} extern "system" fn (recv)(SocketType, *mut c_char, c_int, c_int) -> c_int
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "system" fn(_s: SocketType, buf: *mut c_char, len: c_int, _flags: c_int) -> c_int,
                assign: {
                    let stage = RESPONSE_STAGE.fetch_add(1, Ordering::SeqCst) + 1;
                    if stage <= {
                        MOCK_DELAY_CALLS.with(|c| c.get())
                    } {
                        // Return TLS handshake data for initial calls
                        let copy_len = std::cmp::min(MOCK_TLS_HANDSHAKE.len(), len as usize);
                        std::ptr::copy_nonoverlapping(
                            MOCK_TLS_HANDSHAKE.as_ptr(),
                            buf as *mut u8,
                            copy_len
                        );
                    } else {
                        // Return HTTP response
                        let response_len = MOCK_RESPONSE_DATA_LEN.with(|c| c.get());
                        let response_ptr = MOCK_RESPONSE_DATA_PTR.with(|c| c.get());

                        let copy_len = std::cmp::min(response_len, len as usize);
                        std::ptr::copy_nonoverlapping(
                            response_ptr,
                            buf as *mut u8,
                            copy_len
                        );
                    }
                },
                returns: {
                    let stage = RESPONSE_STAGE.load(Ordering::SeqCst);
                    if stage <= {
                        MOCK_DELAY_CALLS.with(|c| c.get())
                    } {
                        MOCK_TLS_HANDSHAKE.len() as c_int
                    } else {
                        let response_len = MOCK_RESPONSE_DATA_LEN.with(|c| c.get());
                        response_len as c_int
                    }
                }
            ));

        // Mock closesocket to always succeed
        injector
            .when_called(crate::func!(
                unsafe{} extern "system" fn (closesocket)(SocketType) -> c_int
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "system" fn(_s: SocketType) -> c_int,
                returns: 0
            ));
    }

    #[cfg(target_os = "linux")]
    fn install_linux_mocks(&self, injector: &mut InjectorPP) {
        let response_bytes = self.response_bytes.clone();
        let delay_calls = self.config.delay_calls;

        // Mock socket creation
        injector
            .when_called(crate::func!(
                unsafe{} extern "C" fn (socket)(c_int, c_int, c_int) -> c_int
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "C" fn(_domain: c_int, _ty: c_int, _protocol: c_int) -> c_int,
                assign: {
                    let count = SOCKET_COUNT.fetch_add(1, Ordering::SeqCst);
                },
                returns: {
                    let count = SOCKET_COUNT.load(Ordering::SeqCst);
                    (100 + count) as c_int
                }
            ));

        // Mock connect to always succeed
        injector
            .when_called(crate::func!(
                unsafe{} extern "C" fn (connect)(c_int, *const c_void, u32) -> c_int
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "C" fn(_socket: c_int, _address: *const c_void, _len: u32) -> c_int,
                returns: 0
            ));

        // Mock send to always succeed
        injector
            .when_called(crate::func!(
                unsafe{} extern "C" fn (send)(c_int, *const c_void, usize, c_int) -> isize
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "C" fn(_socket: c_int, _buf: *const c_void, len: usize, _flags: c_int) -> isize,
                returns: len as isize
            ));

        // Mock recv to return appropriate data
        let response_clone = response_bytes.clone();
        injector
            .when_called(crate::func!(
                unsafe{} extern "C" fn (recv)(c_int, *mut c_void, usize, c_int) -> isize
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "C" fn(_socket: c_int, buf: *mut c_void, len: usize, _flags: c_int) -> isize,
                assign: {
                    let stage = RESPONSE_STAGE.fetch_add(1, Ordering::SeqCst) + 1;
                    if stage <= delay_calls {
                        // Return TLS handshake data for initial calls
                        let copy_len = std::cmp::min(MOCK_TLS_HANDSHAKE.len(), len);
                        std::ptr::copy_nonoverlapping(
                            MOCK_TLS_HANDSHAKE.as_ptr(),
                            buf as *mut u8,
                            copy_len
                        );
                    } else {
                        // Return HTTP response
                        let copy_len = std::cmp::min(response_clone.len(), len);
                        std::ptr::copy_nonoverlapping(
                            response_clone.as_ptr(),
                            buf as *mut u8,
                            copy_len
                        );
                    }
                },
                returns: {
                    let stage = RESPONSE_STAGE.load(Ordering::SeqCst);
                    if stage <= delay_calls {
                        MOCK_TLS_HANDSHAKE.len() as isize
                    } else {
                        response_clone.len() as isize
                    }
                }
            ));

        // Mock close to always succeed
        injector
            .when_called(crate::func!(
                unsafe{} extern "C" fn (close)(c_int) -> c_int
            ))
            .will_execute(crate::fake!(
                func_type: unsafe extern "C" fn(_fd: c_int) -> c_int,
                returns: 0
            ));
    }
}

/// Convenience macro for creating HTTP mocks
#[macro_export]
macro_rules! http_mock {
    // Simple 200 OK
    () => {
        $crate::http_mock::HttpMocker::ok()
    };

    // Status only
    ($status:expr) => {
        $crate::http_mock::HttpMocker::with_status($status)
    };

    // JSON response
    (json: $json:expr) => {
        $crate::http_mock::HttpMocker::with_json($json)
    };

    // Error response
    (error: $status:expr, $message:expr) => {
        $crate::http_mock::HttpMocker::error($status, $message)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_status_conversion() {
        assert_eq!(HttpStatus::Ok.as_u16(), 200);
        assert_eq!(HttpStatus::NotFound.as_u16(), 404);
        assert_eq!(HttpStatus::InternalServerError.as_u16(), 500);
    }

    #[test]
    fn test_http_mock_config_default() {
        let config = HttpMockConfig::new();
        assert_eq!(config.status.as_u16(), 200);
        assert!(!config.body.is_empty());
        assert!(!config.headers.is_empty());
    }

    #[test]
    fn test_http_mock_config_builder() {
        let config = HttpMockConfig::new()
            .with_status(HttpStatus::NotFound)
            .with_header("X-Custom", "test")
            .with_json_body(r#"{"error": "not found"}"#);

        assert_eq!(config.status.as_u16(), 404);
        assert!(config
            .headers
            .iter()
            .any(|(k, v)| k == "X-Custom" && v == "test"));
        assert_eq!(config.body, r#"{"error": "not found"}"#);
    }

    #[test]
    fn test_response_bytes_generation() {
        let config = HttpMockConfig::new()
            .with_status(HttpStatus::Created)
            .with_json_body(r#"{"id": 123}"#);

        let response_bytes = config.to_response_bytes();
        let response_str = String::from_utf8_lossy(&response_bytes);

        assert!(response_str.starts_with("HTTP/1.1 201 Created"));
        assert!(response_str.contains("Content-Length: 10"));
        assert!(response_str.contains(r#"{"id": 123}"#));
    }
}
