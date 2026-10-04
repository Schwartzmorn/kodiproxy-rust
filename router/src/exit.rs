use axum::response::IntoResponse;

static PANIC_MSG: &str = "Failed to exit server gracefully, panicking...";

pub fn add_exit_route(
    router: axum::Router,
    exit_sender: futures::channel::oneshot::Sender<()>,
) -> axum::Router {
    let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(exit_sender)));
    router.route(
        "/exit",
        axum::routing::get(move || {
            let sender = sender.clone();
            async move {
                match sender.lock().expect(PANIC_MSG).take() {
                    Some(sender) => {
                        sender.send(()).expect(PANIC_MSG);
                        hyper::StatusCode::NO_CONTENT.into_response()
                    }
                    None => (
                        hyper::StatusCode::INTERNAL_SERVER_ERROR,
                        "Server is already shutting down...",
                    )
                        .into_response(),
                }
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use tower::ServiceExt;

    #[tokio::test]
    async fn it_signals_shutdown_and_returns_no_content() {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let router = super::add_exit_route(axum::Router::new(), sender);
        let request = hyper::Request::builder()
            .uri("/exit")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        assert_eq!(204, response.status());
        assert!(receiver.await.is_ok());
    }

    #[tokio::test]
    async fn it_rejects_repeated_shutdown_requests() {
        let (sender, _receiver) = futures::channel::oneshot::channel();
        let router = super::add_exit_route(axum::Router::new(), sender);
        let request = || {
            hyper::Request::builder()
                .uri("/exit")
                .body(axum::body::Body::empty())
                .unwrap()
        };

        assert_eq!(
            204,
            router.clone().oneshot(request()).await.unwrap().status()
        );
        assert_eq!(500, router.oneshot(request()).await.unwrap().status());
    }
}
