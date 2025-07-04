use injectorpp::interface::injector::*;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::TcpStream as StdTcpStream;
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

#[tokio::test]
async fn test_tcp_connect_mock_comprehensive() {
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
