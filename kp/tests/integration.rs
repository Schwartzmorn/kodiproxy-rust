use wiremock::MockServer;

/// This fixture starts a mock server, the server, and cleans the path of the file repository and stops the servers at the end of the test
#[allow(dead_code)]
struct TestFixture {
    mock: MockServer,
    file_path: std::path::PathBuf,
    exit_channel: Option<futures::channel::oneshot::Sender<()>>,
    serve: tokio::task::JoinHandle<()>, // Kept around just in case
}

impl TestFixture {
    pub fn new(file_path: &str, port: u16) -> Self {
        let mock = futures::executor::block_on(wiremock::MockServer::start());
        let configuration = format!(
            r#"{{
            "cec": {{
                "fakeTarget": "{url}"
            }},
            "file": {{
                "rootPath": "{fp}"
            }},
            "jrpc": {{
                "target": "{url}/jsonrpc"
            }},
            "logging": {{
                "level": "WARN"
            }},
            "receiver": {{
                "target": "{url}"
            }},
            "server": {{
                "host": "127.0.0.1:{port}"
            }}
        }}"#,
            url = mock.uri(),
            fp = file_path,
            port = port
        );
        let configuration: kp::configuration::ProxyConfiguration =
            serde_json::from_str(configuration.as_str()).unwrap();
        let file_path = std::path::PathBuf::from(file_path);
        if file_path.exists() {
            std::fs::remove_dir_all(&file_path).expect("Failed to clean the folder before test");
        }

        let (exit_channel, receiver) = futures::channel::oneshot::channel::<()>();

        // I have no clue what I just did
        let rt = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
        let rt_pouet = rt.clone();
        let serve = rt.spawn_blocking(move || {
            let configuration = configuration;
            rt_pouet.block_on(kp::serve_kp(&configuration, Some(receiver)));
        });
        // Wait for the server to have started
        std::thread::sleep(std::time::Duration::from_millis(200));

        TestFixture {
            mock,
            file_path,
            exit_channel: Some(exit_channel),
            serve,
        }
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        let exit_channel = std::mem::replace(&mut self.exit_channel, None).unwrap();

        if let Err(e) = exit_channel.send(()) {
            log::error!("Failed to send the termination signal: {:?}", e);
        };
    }
}

#[rstest::fixture]
fn fixture(#[default("test")] test_name: &str, #[default(8080)] port: u16) -> TestFixture {
    let _ = env_logger::Builder::from_default_env()
        .target(env_logger::Target::Stdout)
        .try_init();
    let file_path = format!("target/test/integration/{}", test_name);
    TestFixture::new(&file_path, port)
}

#[rstest::rstest]
#[tokio::test]
#[allow(unused_variables)]
async fn it_allows_saving_files(#[with("files", 8079)] fixture: TestFixture) {
    let request = hyper::Request::builder()
        .uri(format!("http://127.0.0.1:{}/files/testfile.txt", 8079))
        .method("PUT")
        .body(hyper::Body::from("Fake content"))
        .unwrap();

    let response = hyper::Client::new()
        .request(request)
        .await
        .expect("Error while sending PUT file request");

    let (parts, _) = response.into_parts();

    assert_eq!(201, parts.status);

    let request = hyper::Request::builder()
        .uri(format!("http://127.0.0.1:{}/files/testfile.txt", 8079))
        .method("GET")
        .body(hyper::Body::empty())
        .unwrap();

    let response = hyper::Client::new()
        .request(request)
        .await
        .expect("Error while sending GET file request");

    let (parts, body) = response.into_parts();
    let body = String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap();

    assert_eq!(200, parts.status);
    assert_eq!("Fake content", body.as_str());

    let request = hyper::Request::builder()
        .uri(format!(
            "http://127.0.0.1:{}/file-versions/testfile.txt",
            8079
        ))
        .method("GET")
        .body(hyper::Body::empty())
        .unwrap();

    let response = hyper::Client::new()
        .request(request)
        .await
        .expect("Error while sending GET file request");

    let (parts, body) = response.into_parts();
    let body = String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap();

    assert_eq!(200, parts.status);
    println!("{}", &body);
    let re = regex::Regex::new(r#"^\[\{"timestamp":"[^"]+","address":"127.0.0.1","entry":\{"type":"Creation","version":0,"hash":"X5DLkAP39ZbbRCA79GreR1pKSQNtCJ2iUIugi4/Xpb8="}}]$"#).unwrap();
    assert!(re.is_match(&body));
}

#[rstest::rstest]
#[tokio::test]
async fn it_imbues_jrpc_queries(#[with("jrpc", 8078)] fixture: TestFixture) {
    let receiver_response = r#"<?xml version="1.0" encoding="utf-8" ?>
<item>
<MasterVolume><value>-65</value></MasterVolume>
<Mute><value>on</value></Mute>
</item>"#;

    let request = r#"{"method":"Application.SetVolume", "params": {"volume": 25}}"#;

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/goform/formiPhoneAppVolume.xml"))
        .and(wiremock::matchers::query_param("1 -65.0", ""))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_bytes(String::from(receiver_response)),
        )
        .expect(1)
        .mount(&fixture.mock)
        .await;

    let request = hyper::Request::builder()
        .uri(format!("http://127.0.0.1:{}/jsonrpc", 8078))
        .method("POST")
        .body(hyper::Body::from(request))
        .unwrap();

    let response = hyper::Client::new()
        .request(request)
        .await
        .expect("Error while sending POST volume request");

    let (parts, _) = response.into_parts();
    assert_eq!(200, parts.status);
}
