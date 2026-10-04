#![allow(clippy::double_must_use)]

mod exit;

#[derive(Debug, PartialEq)]
pub enum RouterError {
    ForwardingError(String),
    HandlerError(u16, String),
    InvalidRequest(String),
    MethodNotAllowed,
    NotFound,
}

pub use self::RouterError::*;

use futures::FutureExt;

pub fn parse_url(url: &str) -> (String, String, Option<String>) {
    let url_re: regex::Regex =
        regex::Regex::new(r"^(?P<scheme>https?)://(?P<authority>[^/]+)(?P<path>.*)").unwrap();

    let captures = url_re
        .captures(url)
        .expect("Incorrect url for the jsonrpc server");

    (
        String::from(&captures["scheme"]),
        String::from(&captures["authority"]),
        if !captures["path"].is_empty() {
            Some(String::from(&captures["path"]))
        } else {
            None
        },
    )
}

async fn shutdown_signal(exit_channel: futures::channel::oneshot::Receiver<()>) {
    let mut exit_channel = exit_channel.fuse();

    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c()).fuse();

    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("Could not intercept TERM signal");

    let mut term = Box::pin(term.recv()).fuse();

    futures::select! {
        c = ctrl_c => log::info!("Received Ctrl+C, exiting: {:?}", c),
        r = exit_channel => log::info!("Received exit signal: {:?}", r),
        t = term => log::info!("Received terminate signal: {:?}", t),
    }
}

pub async fn serve(
    host: std::net::SocketAddr,
    exit_channel: Option<futures::channel::oneshot::Receiver<()>>,
    axum_router: axum::Router,
) {
    let (exit_receiver, axum_router) = match exit_channel {
        Some(receiver) => (receiver, axum_router),
        None => {
            let (sender, receiver) = futures::channel::oneshot::channel::<()>();
            (receiver, exit::add_exit_route(axum_router, sender))
        }
    };

    let listener = match tokio::net::TcpListener::bind(host).await {
        Ok(listener) => listener,
        Err(e) => {
            log::error!("server error: {}", e);
            return;
        }
    };

    let graceful = axum::serve(
        listener,
        axum_router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(exit_receiver));

    log::info!("Server now listening on {:?}", host);

    if let Err(e) = graceful.await {
        log::error!("server error: {}", e);
    }

    log::info!("Exiting");
}
