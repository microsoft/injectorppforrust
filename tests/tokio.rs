use injectorpp::interface::injector::*;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Shutdown, TcpListener};
use std::thread;
use std::time::Duration;
use std::{io::Write, net::TcpStream as StdTcpStream};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpSocket, TcpStream};

use http_body_util::BodyExt;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioIo;

use std::net::SocketAddr;
use std::net::ToSocketAddrs;

fn make_unconnected_stream() -> std::io::Result<TcpStream> {
    // 1) create a raw socket
    let sock = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    sock.set_nonblocking(true)?;

    // 2) turn it into a std TcpStream
    let std_stream: StdTcpStream = sock.into();

    // 3) convert that into tokio’s TcpStream
    let tokio_stream = TcpStream::from_std(std_stream)?;
    Ok(tokio_stream)
}

fn make_tcp_with_payload() -> std::io::Result<TcpStream> {
    // 1) bind on 127.0.0.1:0 (OS assigns port)
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let addr = listener.local_addr()?;

    // 2) background “server” writes immediately
    thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            let _ = sock.write_all(b"MOCKED PAYLOAD");
            let _ = sock.shutdown(Shutdown::Write);
        }
    });

    // 3) connect the client (blocking)
    let std_stream = StdTcpStream::connect(addr)?;
    // 4) convert into Tokio TcpStream
    TcpStream::from_std(std_stream)
}

fn make_tcp_with_http_response() -> std::io::Result<TcpStream> {
    // 1) bind on 127.0.0.1:0 (OS assigns port)
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let addr = listener.local_addr()?;

    // 2) background "server" writes a proper HTTP response
    thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            let http_response = b"HTTP/1.1 200 OK\r\n\
                Content-Type: application/json\r\n\
                Content-Length: 87\r\n\
                \r\n\
                {\"url\": \"http://httpbin.org/get\", \"headers\": {\"User-Agent\": \"hyper-test/1.0\"}}";
            let _ = sock.write_all(http_response);
            let _ = sock.shutdown(Shutdown::Write);
        }
    });

    // 3) connect the client (blocking)
    let std_stream = StdTcpStream::connect(addr)?;
    // 4) convert into Tokio TcpStream
    TcpStream::from_std(std_stream)
}

fn make_std_tcp_with_payload(_: &SocketAddr, _: Duration) -> std::io::Result<std::net::TcpStream> {
    // 1) bind on 127.0.0.1:0 (OS assigns port)
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let addr = listener.local_addr()?;

    // 2) background "server" writes immediately
    thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            let _ = sock.write_all(b"MOCKED PAYLOAD");
            let _ = sock.shutdown(Shutdown::Write);
        }
    });

    // 3) connect the client (blocking) and return std::net::TcpStream directly
    let std_stream = StdTcpStream::connect(addr)?;
    Ok(std_stream)
}

#[tokio::test]
async fn test_tokio_tcp_connect_without_payload_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(
            TcpStream::connect(""),
            std::io::Result<TcpStream>
        ))
        .will_return_async(injectorpp::async_return! {
            make_unconnected_stream(),
            std::io::Result<TcpStream>
        });

    let result = TcpStream::connect("192.168.254.254:9999").await;
    let r = result.is_ok();

    if result.is_err() {
        eprintln!("Connection failed: {:?}", result.err());
    }

    assert!(r, "Connection should succeed due to comprehensive mocking");
}

#[tokio::test]
async fn test_tokio_tcp_connect_with_payload_should_success() {
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(
            TcpStream::connect(""),
            std::io::Result<TcpStream>
        ))
        .will_return_async(injectorpp::async_return! {
            make_tcp_with_payload(),
            std::io::Result<TcpStream>
        });

    let result = TcpStream::connect("nonexistwebsite").await;
    let mut stream = result.expect("Connection should succeed due to comprehensive mocking");

    // read all bytes from the mocked stream
    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .expect("failed to read from stream");

    assert_eq!(&buf, b"MOCKED PAYLOAD");
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

    // Create a GET request to httpbin.org (a reliable testing service)
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

    // httpbin.org/get returns JSON with request information
    // Verify that the response contains expected fields
    assert!(
        body_str.contains("\"url\""),
        "Response should contain URL field"
    );
    assert!(
        body_str.contains("httpbin.org/get"),
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

    println!("Response body: {}", body_str);
}
