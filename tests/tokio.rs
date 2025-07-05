use injectorpp::interface::injector::*;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Shutdown, TcpListener};
use std::thread;
use std::{io::Write, net::TcpStream as StdTcpStream};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

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
