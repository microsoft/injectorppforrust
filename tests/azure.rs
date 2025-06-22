use azure_core::Error;
use injectorpp::interface::injector::*;

use azure_core::http::headers::Headers;
use azure_core::http::RawResponse;
use azure_core::http::StatusCode;
use azure_core::http::{new_http_client, Method, Request, Url};

#[tokio::test]
async fn test_azure_http_client_always_return_200() {
    // Create a temporary client + request to capture the method pointer
    let temp_client = new_http_client();
    let mut temp_req = Request::new(Url::parse("https://temp/").unwrap(), Method::Get);

    // Setup the fake
    let mut injector = InjectorPP::new();
    injector
        .when_called_async(injectorpp::async_func!(
            temp_client.execute_request(&mut temp_req),
            std::result::Result<RawResponse, Error>
        ))
        .will_return_async(injectorpp::async_return!(
            // always return an Ok(RawResponse) with status 200
            Ok(RawResponse::from_bytes(StatusCode::Ok, Headers::new(), vec![])),
            std::result::Result<RawResponse, Error>
        ));

    // Run the real code under test
    let client = new_http_client();
    let url = Url::parse("https://nonexistsitetest").unwrap();
    let mut request = Request::new(url, Method::Get);

    let response = client.execute_request(&mut request).await.unwrap();
    assert_eq!(response.status(), 200);
}
