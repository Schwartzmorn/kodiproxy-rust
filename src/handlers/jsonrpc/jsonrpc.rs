/// Trait to implement to override a jsonrpc method
///
/// The method [add_overloader](crate::jsonrpc::JsonrpcHandler::add_overloader()) nust be used to
/// register the overloader
#[async_trait::async_trait]
pub trait JsonrpcOverloader: Sync + Send {
    async fn handle(
        &self,
        parts: hyper::http::request::Parts,
        json_request: JRPCQuery,
        handler: &JsonrpcHandler,
    ) -> Result<JRPCResponse, crate::router::RouterError>;
}

/// Builder for [JsonrpcHandler](crate::jsonrpc::JsonrpcHandler)
pub struct JsonrpcHandlerBuilder {
    authority: String,
    scheme: String,
    overloaders: std::collections::HashMap<String, Box<dyn JsonrpcOverloader>>,
    path: String,
}

/// Sub router dedicated to jsonrpc queries
///
/// Dispatches the different methods to [JsonrpcOverloader](crate::jsonrpc::JsonrpcOverloader) if
/// one is registered to the method, otherwise forwards the query to the actual jsonrpc server
pub struct JsonrpcHandler {
    scheme: String,
    authority: String,
    matcher: Box<dyn crate::router::matcher::Matcher>,
    overloaders: std::collections::HashMap<String, Box<dyn JsonrpcOverloader>>,
    path: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct JRPCQuery {
    jsonrpc: Option<String>,
    method: String,
    params: Option<serde_json::Value>,
    id: Option<i32>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct JRPCResponse {
    jsonrpc: Option<String>,
    result: Option<serde_json::Value>,
    id: Option<i32>,
}

impl JRPCQuery {
    pub fn params(&self) -> &Option<serde_json::Value> {
        &self.params
    }
    pub fn id(&self) -> &Option<i32> {
        &self.id
    }
    pub fn method(&self) -> &String {
        &self.method
    }
    pub fn new(method: String, params: Option<serde_json::Value>, id: Option<i32>) -> JRPCQuery {
        JRPCQuery {
            jsonrpc: Some(String::from("2.0")),
            method,
            params,
            id,
        }
    }
}

impl JRPCResponse {
    pub fn new(result: Option<serde_json::Value>, id: Option<i32>) -> JRPCResponse {
        JRPCResponse {
            jsonrpc: Some(String::from("2.0")),
            result,
            id,
        }
    }

    pub fn result(&self) -> &Option<serde_json::Value> {
        &self.result
    }
}

impl JsonrpcHandlerBuilder {
    /// Gives the path of the jsonrpc service to target (eg. "/jsonrpc")
    pub fn with_path(mut self, path: String) -> JsonrpcHandlerBuilder {
        self.path = if path.len() == 0 || path.starts_with("/") {
            path
        } else {
            String::from("/") + path.as_str()
        };
        self
    }

    /// Gives the scheme (http or https)
    pub fn with_scheme(mut self, scheme: String) -> JsonrpcHandlerBuilder {
        self.scheme = scheme;
        self
    }

    /// Gives the authority to target, ie domain / ip plus optionally the port
    pub fn with_authority(mut self, authority: String) -> JsonrpcHandlerBuilder {
        self.authority = authority;
        self
    }

    /// Gives the full url (optionally the path) to target
    pub fn with_url(mut self, url: String) -> JsonrpcHandlerBuilder {
        let (scheme, authority, path) = crate::router::parse_url(&url);

        self.scheme = scheme;
        self.authority = authority;
        if let Some(path) = path {
            self.path = path;
        }

        self
    }

    /// Adds an overloader
    pub fn add_overloader(
        mut self,
        jrpc_method: String,
        overloader: Box<dyn JsonrpcOverloader>,
    ) -> JsonrpcHandlerBuilder {
        self.overloaders.insert(jrpc_method, overloader);
        self
    }

    /// Builds the [JsonrpcHandler](crate::jsonrpc::JsonrpcHandler)
    pub fn build(self) -> Box<JsonrpcHandler> {
        Box::from(JsonrpcHandler {
            scheme: self.scheme,
            authority: self.authority,
            matcher: crate::router::matcher::builder()
                .exact_path(&self.path)
                .build()
                .unwrap(),
            overloaders: self.overloaders,
            path: self.path,
        })
    }
}

impl JsonrpcHandler {
    pub fn builder() -> JsonrpcHandlerBuilder {
        JsonrpcHandlerBuilder {
            authority: String::from("127.0.0.1:8080"),
            scheme: String::from("http"),
            overloaders: std::collections::HashMap::new(),
            path: String::from("/jsonrpc"),
        }
    }

    /// Forwards the request to the actual jsonrpc server
    pub async fn forward(
        &self,
        parts: hyper::http::request::Parts,
        body: hyper::body::Bytes,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        let uri = hyper::Uri::builder()
            .scheme(self.scheme.as_str())
            .authority(self.authority.as_str())
            .path_and_query(self.path.as_str())
            .build()
            .unwrap();

        let mut request_builder = hyper::Request::builder()
            .method(parts.method)
            .uri(uri)
            .version(parts.version);
        request_builder.headers_mut().unwrap().extend(parts.headers);

        let request = request_builder
            .body(hyper::body::Body::from(body))
            .map_err(|err| {
                JsonrpcHandler::f_err("Error while building the forwarding jsonrpc request", &err)
            })?;

        hyper::Client::new()
            .request(request)
            .await
            .map_err(|err| JsonrpcHandler::f_err("Error while forwarding jsonrpc request", &err))
    }

    pub async fn forward_jrpc(
        &self,
        parts: hyper::http::request::Parts,
        query: JRPCQuery,
    ) -> Result<JRPCResponse, crate::router::RouterError> {
        let body = hyper::body::Bytes::from(serde_json::to_string(&query).unwrap());
        let result = self.forward(parts, body).await?;
        // TODO: better error handling
        let body = result.into_body();

        let body = hyper::body::to_bytes(body)
            .await
            .map_err(|e| JsonrpcHandler::h_err("Could not read body of jsonrpc response", &e))?;

        let body_str = String::from_utf8(body.to_vec())
            .map_err(|e| JsonrpcHandler::h_err("Jsonrpc response body is not valid utf-8", &e))?;

        let json: JRPCResponse = serde_json::from_str(body_str.as_str())
            .map_err(|e| JsonrpcHandler::h_err("Jsonrpc response body is not valid json", &e))?;

        Ok(json)
    }

    fn f_err<T: std::fmt::Display>(msg: &str, err: &T) -> crate::router::RouterError {
        // TODO log
        let msg = format!("{}: [{}]", msg, err);
        eprintln!("{}", msg);
        crate::router::RouterError::ForwardingError(msg)
    }

    fn h_err<T: std::fmt::Display>(msg: &str, err: &T) -> crate::router::RouterError {
        // TODO log
        let msg = format!("{}: [{}]", msg, err);
        eprintln!("{}", msg);
        crate::router::RouterError::InvalidRequest(msg)
    }
}

#[async_trait::async_trait]
impl crate::router::Handler for JsonrpcHandler {
    fn get_matcher(&self) -> &Box<dyn crate::router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        let (parts, body) = request.into_parts();
        let body = hyper::body::to_bytes(body)
            .await
            .map_err(|e| JsonrpcHandler::h_err("Could not read body of jsonrpc request", &e))?;

        let body_str = String::from_utf8(body.to_vec())
            .map_err(|e| JsonrpcHandler::h_err("Jsonrpc request body is not valid utf-8", &e))?;

        if &parts.method == hyper::Method::POST {
            let json: JRPCQuery = serde_json::from_str(body_str.as_str())
                .map_err(|e| JsonrpcHandler::h_err("Jsonrpc request body is not valid json", &e))?;

            if let Some(overloader) = self.overloaders.get(json.method()) {
                if json.params().is_none() {
                    return Err(JsonrpcHandler::h_err(
                        "Jsonrpc request did not contain any parameter",
                        json.method(),
                    ));
                }
                // TODO improve this with better error handling
                // TODO improve deserialization
                return overloader.handle(parts, json, self).await.map(|response| {
                    hyper::Response::builder()
                        .status(200)
                        .header("content-type", "application/json")
                        .body(hyper::Body::from(serde_json::to_string(&response).unwrap()))
                        .unwrap()
                });
            }
        }
        // when in doubt, forward
        self.forward(parts, body).await
    }
}

#[cfg(test)]
mod tests {
    use crate::handlers::jsonrpc::JsonrpcOverloader;
    use crate::router::Handler;

    struct MockOverloader {}

    #[async_trait::async_trait]
    impl JsonrpcOverloader for MockOverloader {
        async fn handle(
            &self,
            _parts: hyper::http::request::Parts,
            _body: crate::handlers::jsonrpc::JRPCQuery,
            _handler: &super::JsonrpcHandler,
        ) -> Result<super::JRPCResponse, crate::router::RouterError> {
            Ok(super::JRPCResponse::new(None, Some(1)))
        }
    }

    #[tokio::test]
    async fn it_forwards_when_no_overloader() {
        let mock_server: wiremock::MockServer = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/jsonrpc"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes("a post body"))
            .mount(&mock_server)
            .await;

        let jrpc = crate::handlers::jsonrpc::JsonrpcHandler::builder()
            .with_url(mock_server.uri())
            .build();

        let req = hyper::Request::builder()
            .uri("/jsonrpc")
            .method("POST")
            .body(hyper::Body::from(r#"{"method":"Not.Found"}"#))
            .unwrap();

        let (parts, body) = jrpc.handle(req).await.unwrap().into_parts();

        assert_eq!(200, parts.status);

        let body = String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap();

        assert_eq!("a post body", body);
    }

    #[tokio::test]
    async fn it_returns_errors() {
        let jrpc = crate::handlers::jsonrpc::JsonrpcHandler::builder().build();

        let req = hyper::Request::builder()
            .uri("/jsonrpc")
            .method("POST")
            .body(hyper::Body::from(r#"invalidjson"#))
            .unwrap();

        let error = jrpc.handle(req).await.unwrap_err();

        match error {
            crate::router::RouterError::InvalidRequest(msg) => {
                assert!(msg.starts_with("Jsonrpc request body is not valid json"))
            }
            _ => panic!("Wrong type of error"),
        }
    }

    #[tokio::test]
    async fn it_forwards_to_overloader() {
        let jrpc = crate::handlers::jsonrpc::JsonrpcHandler::builder()
            .add_overloader(String::from("A.Method"), Box::from(MockOverloader {}))
            .build();

        let req = hyper::Request::builder()
            .uri("/jsonrpc")
            .method("POST")
            .body(hyper::Body::from(r#"{"method":"A.Method"}"#))
            .unwrap();

        let (parts, body) = jrpc.handle(req).await.unwrap().into_parts();

        assert_eq!(200, parts.status);

        let body = String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap();

        assert_eq!(r#"{"jsonrpc":"2.0","result":null,"id":1}"#, body);
    }

    #[tokio::test]
    async fn it_forwards_jrpc() {
        let mock_server: wiremock::MockServer = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/jsonrpc"))
            .and(wiremock::matchers::body_string(
                r#"{"jsonrpc":"2.0","method":"a.method","params":{"akey":"a value"},"id":42}"#,
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(r#"{"result":{"res":"a result"}}"#),
            )
            .mount(&mock_server)
            .await;

        let jrpc = crate::handlers::jsonrpc::JsonrpcHandler::builder()
            .with_url(mock_server.uri())
            .build();

        let req = hyper::Request::builder()
            .uri("/jsonrpc")
            .method("POST")
            .body(hyper::Body::empty())
            .unwrap();

        let (parts, _) = req.into_parts();

        let query = super::JRPCQuery::new(
            String::from("a.method"),
            Some(serde_json::json!({"akey": "a value"})),
            Some(42),
        );

        let res = jrpc.forward_jrpc(parts, query).await.unwrap();
        let res = res.result().to_owned().unwrap();

        assert_eq!(serde_json::json!({"res":"a result"}), res);
    }
}
