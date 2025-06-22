use injectorpp::interface::injector::*;

async fn simple_async_func_u32_add_one(x: u32) -> u32 {
    x + 1
}

async fn simple_async_func_u32_add_two(x: u32) -> u32 {
    x + 2
}

async fn simple_async_func_bool(x: bool) -> bool {
    x
}

struct HttpClientTest {
    pub url: String,
}

impl HttpClientTest {
    pub async fn get(&self) -> String {
        format!("GET {}", self.url)
    }

    pub async fn post(&self, payload: &str) -> String {
        format!("POST {} to {}", payload, self.url)
    }
}

#[tokio::test]
async fn test_simple_async_unchecked() {
    let mut injector = InjectorPP::new();

    unsafe {
        injector
            .when_called_async_unchecked(injectorpp::async_func_unchecked!(
                simple_async_func_u32_add_one(u32::default())
            ))
            .will_return_async_unchecked(injectorpp::async_return_unchecked!(123, u32));
    }

    let x = simple_async_func_u32_add_one(1).await;
    assert_eq!(x, 123);

    // other async function unaffected
    let x2 = simple_async_func_u32_add_two(1).await;
    assert_eq!(x2, 3);

    unsafe {
        injector
            .when_called_async_unchecked(injectorpp::async_func_unchecked!(simple_async_func_bool(
                bool::default()
            )))
            .will_return_async_unchecked(injectorpp::async_return_unchecked!(false, bool));
    }

    let y = simple_async_func_bool(true).await;
    assert_eq!(y, false);
}

#[tokio::test]
async fn test_complex_struct_async_unchecked() {
    // Test GET method
    {
        let temp_client = HttpClientTest {
            url: String::default(),
        };

        let mut injector = InjectorPP::new();
        unsafe {
            injector
                .when_called_async_unchecked(injectorpp::async_func_unchecked!(temp_client.get()))
                .will_return_async_unchecked(injectorpp::async_return_unchecked!(
                    "Fake GET response".to_string(),
                    String
                ));
        }

        let real_client = HttpClientTest {
            url: "https://test.com".to_string(),
        };

        let result = real_client.get().await;
        assert_eq!(result, "Fake GET response".to_string());
    }

    // After injector dropped, original behavior restored
    let real_client = HttpClientTest {
        url: "https://test.com".to_string(),
    };

    let result = real_client.get().await;
    assert_eq!(result, "GET https://test.com".to_string());

    // Test POST method
    {
        let temp_client = HttpClientTest {
            url: String::default(),
        };

        let mut injector = InjectorPP::new();
        unsafe {
            injector
                .when_called_async_unchecked(injectorpp::async_func_unchecked!(
                    temp_client.post("payload")
                ))
                .will_return_async_unchecked(injectorpp::async_return_unchecked!(
                    "Fake POST response".to_string(),
                    String
                ));
        }

        let real_client = HttpClientTest {
            url: "https://test.com".to_string(),
        };

        let result = real_client.post("payload").await;
        assert_eq!(result, "Fake POST response".to_string());
    }

    // After injector dropped, original behavior restored
    let real_client = HttpClientTest {
        url: "https://test.com".to_string(),
    };

    let result = real_client.post("payload").await;
    assert_eq!(result, "POST payload to https://test.com".to_string());
}
