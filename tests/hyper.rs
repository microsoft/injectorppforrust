use injectorpp::interface::injector::*;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::thread;
use std::{io::Write, net::TcpStream as StdTcpStream};
use tokio::net::{TcpSocket, TcpStream};

use http_body_util::BodyExt;
use hyper::Request;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;

use std::net::SocketAddr;
use std::net::ToSocketAddrs;

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
                r#"{"url": "http://nonexistwebsite", "headers": {"User-Agent": "hyper-test/1.0"}}"#;
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

    // 4) convert into Tokio TcpStream
    TcpStream::from_std(std_stream)
}

#[tokio::test]
async fn test_hyper_real_http_request() {
    let mut injector = InjectorPP::new();

    type ToSocketAddrsFn =
        fn(&(&'static str, u16)) -> std::io::Result<std::vec::IntoIter<SocketAddr>>;
    let fn_ptr: ToSocketAddrsFn = <(&'static str, u16) as ToSocketAddrs>::to_socket_addrs;

    unsafe {
        injector
            .when_called_unchecked(injectorpp::func_unchecked!(fn_ptr))
            .will_execute_raw_unchecked(injectorpp::closure_unchecked!(
                |_addr: &(&str, u16)| -> std::io::Result<std::vec::IntoIter<SocketAddr>> {
                    Ok(vec![SocketAddr::from(([127, 0, 0, 1], 1))].into_iter())
                },
                fn(&(&str, u16)) -> std::io::Result<std::vec::IntoIter<SocketAddr>>
            ));
    }

    let temp_socket = TcpSocket::new_v4().expect("Failed to create temp socket");
    let temp_addr = "127.0.0.1:80".parse().unwrap();

    // Mock TcpSocket::connect method
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

    let request = Request::builder()
        .method("GET")
        .uri("http://nonexistwebsite")
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
        body_str.contains("\"url\""),
        "Response should contain URL field"
    );

    assert!(
        body_str.contains("nonexistwebsite"),
        "Response should contain the requested URL"
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
