static PANIC_MSG: &'static str = "Failed to exit server gracefully, panicing...";

struct ExitHandler {
    matcher: Box<dyn crate::router::matcher::Matcher>,
    sender: std::sync::Mutex<Option<futures::channel::oneshot::Sender<()>>>,
}

#[async_trait::async_trait]
impl crate::router::Handler for ExitHandler {
    fn get_matcher(&self) -> &Box<dyn crate::router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        _request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        // This panics if we can't get the lock or if the channel has already been used
        self.sender
            .lock()
            .expect(PANIC_MSG)
            .take()
            .ok_or(crate::router::RouterError::HandlerError(
                500,
                String::from("Server is already shutting down..."),
            ))?
            .send(())
            .expect(PANIC_MSG);

        Ok(hyper::Response::builder()
            .status(204)
            .body(hyper::Body::empty())
            .unwrap())
    }
}

pub fn get_handler(
    exit_sender: futures::channel::oneshot::Sender<()>,
) -> Box<dyn crate::router::Handler> {
    let matcher = crate::router::matcher::builder()
        .exact_path(String::from("/exit"))
        .with_method(&hyper::Method::GET)
        .build()
        .unwrap();
    Box::from(ExitHandler {
        matcher,
        sender: std::sync::Mutex::new(Some(exit_sender)),
    })
}
