// These tests use IP addresses (127.0.0.1) to avoid DNS resolution, which
// requires x86_64's thread-local dispatch. On ARM32, the removed to_socket_addrs
// fake causes the test to hang because the TcpSocket::connect fake may not
// intercept hyper's internal connect call correctly.
#![cfg(target_arch = "x86_64")]
use hyper::Uri;
use injectorpp::interface::injector::*;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::thread;
use std::{io::Write, net::TcpStream as StdTcpStream};
use tokio::net::{TcpSocket, TcpStream};

// Mock TCP stream that provides an HTTP response
fn make_tcp_with_json_response() -> std::io::Result<TcpStream> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let addr = listener.local_addr()?;

    thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            // Read the HTTP request first
            let mut reader = BufReader::new(&mut sock);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_ok() {
                // Read headers until empty line
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok() && line.trim() != "" {
                    line.clear();
                }
            }

            let json_body = r#"{
                "status": "success",
                "message": "Hello from injectorpp!",
                "data": {
                    "user_id": 12345,
                    "username": "test_user",
                    "email": "test@example.com"
                },
                "timestamp": "2025-07-05T10:30:00Z"
            }"#;

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                Content-Type: application/json\r\n\
                Content-Length: {}\r\n\
                Server: MockServer/1.0\r\n\
                X-Custom-Header: test-value\r\n\
                Connection: close\r\n\
                \r\n\
                {}",
                json_body.len(),
                json_body
            );

            let _ = sock.write_all(response.as_bytes());
            let _ = sock.flush();
            let _ = sock.shutdown(std::net::Shutdown::Write);
        }
    });

    let std_stream = StdTcpStream::connect(addr)?;
    std_stream.set_nonblocking(true)?;
    TcpStream::from_std(std_stream)
}

#[tokio::test]
async fn test_reqwest_get_https_request_with_json_response() {
    let mut injector = InjectorPP::new();

    let temp_socket = TcpSocket::new_v4().expect("Failed to create temp socket");
    let temp_addr = "127.0.0.1:0".parse().unwrap();

    // Use injectorpp to fake TcpSocket::connect so reqwest connects to our
    // local mock server instead of making a real network request.
    injector
        .when_called_async(injectorpp::async_func!(
            temp_socket.connect(temp_addr),
            std::io::Result<TcpStream>
        ))
        .will_return_async(injectorpp::async_return! {
            make_tcp_with_json_response(),
            std::io::Result<TcpStream>
        });

    // Use injectorpp to fake Uri::scheme_str to return "http", bypassing
    // TLS validation while still using reqwest's default HTTPS handling.
    injector
        .when_called(injectorpp::func!(fn (Uri::scheme_str)(&Uri) -> Option<&str>))
        .will_execute(injectorpp::fake!(
            func_type: fn(_uri: &Uri) -> Option<&str>,
            returns: Some("http")
        ));

    // Simulated reqwest client creation and request.
    // Use an IP address to avoid DNS resolution (which runs on a separate
    // thread and cannot be intercepted by injectorpp's thread-local dispatch).
    let client = reqwest::Client::new();

    // Execute the request
    let response = client
        .get("http://127.0.0.1")
        .header("User-Agent", "reqwest-test/1.0")
        .header("Accept", "application/json")
        .send()
        .await
        .expect("Failed to send request");

    // Verify response status
    assert_eq!(response.status(), 200, "Expected status code 200");
    assert!(
        response.status().is_success(),
        "Expected successful response"
    );

    // Verify response headers
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json",
        "Expected JSON content type"
    );

    // Verify response body
    let body = response.text().await.expect("Failed to read response body");

    assert!(
        body.contains("Hello from injectorpp!"),
        "Expected message in response body"
    );
    assert!(
        body.contains("test_user"),
        "Expected username in response body"
    );
    assert!(
        body.contains("test@example.com"),
        "Expected email in response body"
    );
}
