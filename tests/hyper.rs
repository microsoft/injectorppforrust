use hyper_tls::HttpsConnector;
use injectorpp::interface::injector::*;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::thread;
use std::{io::Write, net::TcpStream as StdTcpStream};
use tokio::net::{TcpSocket, TcpStream};

use http_body_util::BodyExt;
use hyper::{Request, Uri};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;

fn make_tcp_with_http_response() -> std::io::Result<TcpStream> {
    // 1) bind on 127.0.0.1:0 (OS assigns port)
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let addr = listener.local_addr()?;

    // 2) background "server" writes a proper HTTP response
    thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            // Read the HTTP request first (important for proper HTTP flow)
            let mut reader = BufReader::new(&mut sock);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_ok() {
                // Read headers until empty line
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok() && line.trim() != "" {
                    line.clear();
                }
            }

            let body =
                r#"{"status": "ok", "message": "mock response", "headers": {"User-Agent": "hyper-test/1.0"}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                Content-Type: application/json\r\n\
                Content-Length: {}\r\n\
                Connection: close\r\n\
                \r\n\
                {}",
                body.len(),
                body
            );

            let _ = sock.write_all(response.as_bytes());
            let _ = sock.flush();
            let _ = sock.shutdown(std::net::Shutdown::Write);
        }
    });

    // 3) connect the client (blocking)
    let std_stream = StdTcpStream::connect(addr)?;

    // 4) IMPORTANT: Set the socket to non-blocking before converting to Tokio
    std_stream.set_nonblocking(true)?;

    // 5) convert into Tokio TcpStream
    TcpStream::from_std(std_stream)
}

#[tokio::test]
async fn test_hyper_http_request() {
    let mut injector = InjectorPP::new();

    let temp_socket = TcpSocket::new_v4().expect("Failed to create temp socket");
    let temp_addr = "127.0.0.1:80".parse().unwrap();

    // Use injectorpp to fake TcpSocket::connect so hyper connects to our
    // local mock server instead of making a real network request.
    injector
        .when_called_async(injectorpp::async_func!(
            temp_socket.connect(temp_addr),
            std::io::Result<TcpStream>
        ))
        .will_return_async(injectorpp::async_return! {
            make_tcp_with_http_response(),
            std::io::Result<TcpStream>
        });

    // Create a hyper client
    let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(HttpConnector::new());

    // Use an IP address to avoid DNS resolution (which runs on a separate
    // thread and cannot be intercepted by injectorpp's thread-local dispatch).
    let request = Request::builder()
        .method("GET")
        .uri("http://127.0.0.1")
        .header("User-Agent", "hyper-test/1.0")
        .body(String::new())
        .expect("Failed to build request");

    // Send the request and get the response
    let response = client
        .request(request)
        .await
        .expect("Failed to send request");

    // Check that we got a successful response
    assert!(
        response.status().is_success(),
        "Expected successful response"
    );
    assert_eq!(response.status().as_u16(), 200, "Expected status code 200");

    // Read the response body
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("Failed to read response body")
        .to_bytes();

    let body_str =
        String::from_utf8(body_bytes.to_vec()).expect("Failed to convert response body to string");

    assert!(
        body_str.contains("\"status\""),
        "Response should contain status field"
    );

    assert!(
        body_str.contains("mock response"),
        "Response should contain mock message"
    );

    assert!(
        body_str.contains("\"headers\""),
        "Response should contain headers field"
    );

    assert!(
        body_str.contains("hyper-test/1.0"),
        "Response should contain our User-Agent"
    );
}

#[tokio::test]
async fn test_hyper_https_request() {
    let mut injector = InjectorPP::new();

    let temp_socket = TcpSocket::new_v4().expect("Failed to create temp socket");
    let temp_addr = "127.0.0.1:80".parse().unwrap();

    // Use injectorpp to fake TcpSocket::connect so hyper connects to our
    // local mock server instead of making a real network request.
    injector
        .when_called_async(injectorpp::async_func!(
            temp_socket.connect(temp_addr),
            std::io::Result<TcpStream>
        ))
        .will_return_async(injectorpp::async_return! {
            make_tcp_with_http_response(),
            std::io::Result<TcpStream>
        });

    // Use injectorpp to fake Uri::scheme_str to return "http", bypassing
    // TLS validation while still using HttpsConnector.
    injector
        .when_called(injectorpp::func!(fn (Uri::scheme_str)(&Uri) -> Option<&str>))
        .will_execute(injectorpp::fake!(
            func_type: fn(_uri: &Uri) -> Option<&str>,
            returns: Some("http")
        ));

    // Create a hyper client with HTTPS connector
    let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(HttpsConnector::new());

    // Use an IP address to avoid DNS resolution (which runs on a separate
    // thread and cannot be intercepted by injectorpp's thread-local dispatch).
    // The scheme_str fake above downgrades HTTPS to HTTP transparently.
    let request = Request::builder()
        .method("GET")
        .uri("https://127.0.0.1")
        .header("User-Agent", "hyper-test/1.0")
        .body(String::new())
        .expect("Failed to build request");

    // Send the request and get the response
    let response = client
        .request(request)
        .await
        .expect("Failed to send request");

    // Check that we got a successful response
    assert!(
        response.status().is_success(),
        "Expected successful response"
    );
    assert_eq!(response.status().as_u16(), 200, "Expected status code 200");

    // Read the response body
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("Failed to read response body")
        .to_bytes();

    let body_str =
        String::from_utf8(body_bytes.to_vec()).expect("Failed to convert response body to string");

    assert!(
        body_str.contains("\"status\""),
        "Response should contain status field"
    );

    assert!(
        body_str.contains("mock response"),
        "Response should contain mock message"
    );

    assert!(
        body_str.contains("\"headers\""),
        "Response should contain headers field"
    );

    assert!(
        body_str.contains("hyper-test/1.0"),
        "Response should contain our User-Agent"
    );
}
