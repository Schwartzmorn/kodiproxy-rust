pub mod avreceiver;
pub mod handlers;
pub mod router;

async fn shutdown_signal() {
    // TODO wait for signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
}

#[tokio::main]
async fn main() {
    // We'll bind to 127.0.0.1:3000
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));

    let mut router = router::Router::new();
    let handler = handlers::jsonrpc::JsonrpcHandler::builder()
        .with_url(String::from("http://192.168.1.18:8081/jsonrpc"))
        .build();
    router.add_handler(handler);

    let router = std::sync::Arc::new(router);

    let make_svc = hyper::service::make_service_fn(move |_conn| {
        let router = router.clone();
        async move {
            Ok::<_, std::convert::Infallible>(hyper::service::service_fn(move |req| {
                let router = router.clone();
                async move { router.handle(req).await }
            }))
        }
    });

    let server = hyper::Server::bind(&addr).serve(make_svc);

    let graceful = server.with_graceful_shutdown(shutdown_signal());

    // Run this server for... forever!
    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }
}
