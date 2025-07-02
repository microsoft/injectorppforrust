use injectorpp::interface::injector::*;
use injectorpp::utilities::network::*;

#[tokio::test]
async fn test_hyper_client_simple_200_ok() {
    let mut injector = InjectorPP::new();

    // Simple 200 OK response
    let mocker = HttpMocker::ok();
    mocker.install(&mut injector);

    // Test with hyper client
    use http_body_util::Empty;
    use hyper::{Request, Uri};
    use hyper_tls::HttpsConnector;
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;

    let https = HttpsConnector::new();
    let client = Client::builder(TokioExecutor::new()).build(https);

    let uri: Uri = "https://httpbin.org/get".parse().unwrap();
    let req = Request::builder()
        .uri(uri)
        .header("User-Agent", "hyper-test/1.0")
        .body(Empty::<hyper::body::Bytes>::new())
        .unwrap();

    let response = client.request(req).await.unwrap();
    assert_eq!(response.status(), 200);
    println!("✅ Simple 200 OK test passed!");
}
