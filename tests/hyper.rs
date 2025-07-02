use injectorpp::interface::injector::*;

#[tokio::test]
async fn test_hyper_client_simple_200_ok() {
    use http_body_util::Empty;
    use hyper::{Request, Response, StatusCode, Uri};
    use hyper_tls::HttpsConnector;
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;

    // Create a temporary client and request to capture the method signature
    let https = HttpsConnector::new();
    let temp_client = Client::builder(TokioExecutor::new()).build(https.clone());
    let temp_uri: Uri = "https://temp.com/".parse().unwrap();
    let temp_req = Request::builder()
        .uri(temp_uri)
        .body(Empty::<hyper::body::Bytes>::new())
        .unwrap();

    // Create the injector and mock the request method
    let mut injector = InjectorPP::new();

    // Mock the client's request method to always return 200 OK
    unsafe {
        injector
            .when_called_async_unchecked(injectorpp::async_func_unchecked!(
                temp_client.request(temp_req)
            ))
            .will_return_async_unchecked(injectorpp::async_return_unchecked!(
                Ok({
                    let mock_body = Empty::<hyper::body::Bytes>::new();
                    let mock_response = Response::builder()
                        .status(StatusCode::OK)
                        .body(mock_body)
                        .unwrap();
                    mock_response
                }),
                Result<Response<Empty<hyper::body::Bytes>>, hyper_util::client::legacy::Error>
            ));
    }

    // Now test with the actual non-existent host - it should return 200 due to mocking
    let client = Client::builder(TokioExecutor::new()).build(https);
    let uri: Uri = "https://nonexistsite.com/get".parse().unwrap();
    let req = Request::builder()
        .uri(uri)
        .header("User-Agent", "hyper-test/1.0")
        .body(Empty::<hyper::body::Bytes>::new())
        .unwrap();

    let response = client.request(req).await.unwrap();

    assert_eq!(response.status(), 200);
}
