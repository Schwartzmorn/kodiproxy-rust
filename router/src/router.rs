use crate::matcher::MatcherResult;

pub use self::RouterError::*;

#[async_trait::async_trait]
pub trait Handler: Sync + Send {
    fn get_matcher(&self) -> &Box<dyn crate::matcher::Matcher>;
    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, RouterError>;
}

#[derive(Debug, PartialEq)]
pub enum RouterError {
    ForwardingError(String),
    HandlerError(u16, String),
    InvalidRequest(String),
    MethodNotAllowed,
    NotFound,
}

pub struct Router {
    handlers: Vec<Box<dyn Handler>>,
}

impl Router {
    pub fn new() -> Router {
        Router {
            handlers: Vec::new(),
        }
    }

    pub fn add_handler(&mut self, handler: Box<dyn Handler>) -> &mut Self {
        self.handlers.push(handler);
        self
    }

    pub fn add_handlers<T>(&mut self, handlers: T) -> &mut Self
    where
        T: IntoIterator<Item = Box<dyn Handler>>,
    {
        for handler in handlers {
            self.handlers.push(handler);
        }
        self
    }

    pub async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, std::convert::Infallible> {
        // TODO improve this when async closure are supported
        match self.get_handler(&request) {
            Ok(handler) => {
                // TODO improve timeout mechanism ?
                let duration = std::time::Duration::from_secs(5);
                match async_std::future::timeout(duration, handler.handle(request))
                    .await
                    .map_err(|_| RouterError::HandlerError(504, String::from("Handler time outed")))
                {
                    // TODO improve when Result::flatten() is stable
                    Ok(response) => match response {
                        Ok(response) => Ok(response),
                        Err(err) => Router::error(err),
                    },
                    Err(err) => Router::error(err),
                }
            }
            Err(error) => Router::error(error),
        }
    }

    fn get_handler(
        &self,
        request: &hyper::Request<hyper::Body>,
    ) -> Result<&Box<dyn Handler>, RouterError> {
        log::info!("{:?} {:?}", request.method(), request.uri());
        log::trace!("Headers: {:?}", request.headers());
        let mut server_error = RouterError::NotFound;
        for handler in self.handlers.iter() {
            match handler.get_matcher().matches(request) {
                MatcherResult::OK => return Ok(handler),
                MatcherResult::UriOnly => server_error = RouterError::MethodNotAllowed,
                MatcherResult::KO => (),
            }
        }
        Err(server_error)
    }

    fn error(error: RouterError) -> Result<hyper::Response<hyper::Body>, std::convert::Infallible> {
        log::info!("Sending error response {:?}", &error);
        Ok(hyper::Response::builder()
            .status(match &error {
                RouterError::ForwardingError(_) => 502,
                RouterError::HandlerError(status, _) => *status,
                RouterError::InvalidRequest(_) => 400,
                RouterError::MethodNotAllowed => 405,
                RouterError::NotFound => 404,
            })
            .header("content-type", "text/plain")
            .body(hyper::Body::from(match error {
                RouterError::ForwardingError(msg) => msg,
                RouterError::HandlerError(_, msg) => msg,
                RouterError::InvalidRequest(msg) => msg,
                RouterError::MethodNotAllowed => String::from("Method Not Allowed"),
                RouterError::NotFound => String::from("Not Found"),
            }))
            .unwrap())
    }
}

#[cfg(test)]
mod tests {
    struct MockHandler {
        matcher: Box<dyn crate::matcher::Matcher>,
        wait: u64,
    }

    impl MockHandler {
        pub fn new(wait: u64) -> MockHandler {
            MockHandler {
                matcher: crate::matcher::builder()
                    .exact_path("/jsonrpc")
                    .with_method("GET")
                    .build()
                    .unwrap(),
                wait,
            }
        }
    }

    #[async_trait::async_trait]
    impl super::Handler for MockHandler {
        fn get_matcher(&self) -> &Box<dyn crate::matcher::Matcher> {
            &self.matcher
        }
        async fn handle(
            &self,
            _request: hyper::Request<hyper::Body>,
        ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
            async_std::task::sleep(std::time::Duration::from_secs(self.wait)).await;
            Ok(hyper::Response::builder()
                .status(200)
                .body(hyper::Body::from("a response"))
                .unwrap())
        }
    }

    fn get_request(uri: &str, method: &hyper::Method) -> hyper::Request<hyper::Body> {
        hyper::Request::builder()
            .uri(uri)
            .method(method)
            .body(hyper::Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn it_routes() {
        let mut router = super::Router::new();
        router.add_handler(Box::new(MockHandler::new(0)));

        let request = get_request("/jsonrpc", &hyper::Method::GET);
        let (parts, body) = router.handle(request).await.unwrap().into_parts();

        let body = hyper::body::to_bytes(body).await.unwrap();

        assert_eq!(200, parts.status);
        assert_eq!("a response", body);
    }

    #[tokio::test]
    async fn it_answers_404_when_no_handler() {
        let mut router = super::Router::new();
        router.add_handler(Box::new(MockHandler::new(0)));

        let request = get_request("/not_found", &hyper::Method::GET);
        let (parts, _) = router.handle(request).await.unwrap().into_parts();

        assert_eq!(404, parts.status);
    }

    #[tokio::test]
    async fn it_answers_405() {
        let mut router = super::Router::new();
        router.add_handler(Box::new(MockHandler::new(0)));

        let request = get_request("/jsonrpc", &hyper::Method::POST);
        let (parts, _) = router.handle(request).await.unwrap().into_parts();

        assert_eq!(405, parts.status);
    }

    #[tokio::test]
    async fn it_answers_504_when_handler_timeouts() {
        // TODO speed up this test if timeout improves
        let mut router = super::Router::new();
        router.add_handler(Box::new(MockHandler::new(6)));

        let request = get_request("/jsonrpc", &hyper::Method::GET);
        let (parts, _) = router.handle(request).await.unwrap().into_parts();

        assert_eq!(504, parts.status);
    }
}
