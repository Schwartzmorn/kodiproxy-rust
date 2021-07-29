pub use self::router::*;
mod exit;
pub mod matcher;
pub mod router;

use futures::FutureExt;

pub fn parse_url(url: &String) -> (String, String, Option<String>) {
    let url_re: regex::Regex =
        regex::Regex::new(r"^(?P<scheme>https?)://(?P<authority>[^/]+)(?P<path>.*)").unwrap();

    let captures = url_re
        .captures(url.as_str())
        .expect("Incorrect url for the jsonrpc server");

    (
        String::from(&captures["scheme"]),
        String::from(&captures["authority"]),
        if captures["path"].len() > 0 {
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

pub async fn serve<F>(
    host: std::net::SocketAddr,
    exit_channel: Option<futures::channel::oneshot::Receiver<()>>,
    register_handlers: F,
) where
    F: FnOnce(&mut Router),
{
    let mut exit_sender: Option<futures::channel::oneshot::Sender<()>> = None;

    let exit_receiver = match exit_channel {
        Some(receiver) => receiver,
        None => {
            // this ultimately means the "quit" handler is only setup if the receiver is not given in input
            let (sender, receiver) = futures::channel::oneshot::channel::<()>();
            exit_sender = Some(sender);
            receiver
        }
    };

    let mut router = router::Router::new();
    if let Some(exit_sender) = exit_sender {
        router.add_handler(exit::get_handler(exit_sender));
    }
    register_handlers(&mut router);
    let router = std::sync::Arc::new(router);

    let make_svc =
        hyper::service::make_service_fn(move |connection: &hyper::server::conn::AddrStream| {
            let remote_address = connection.remote_addr();

            match remote_address {
                std::net::SocketAddr::V4(addr) => {
                    log::debug!("Got connection from ipv4 {:?}", addr.ip());
                }
                std::net::SocketAddr::V6(addr) => {
                    log::debug!("Got connection from ipv6 {:?}", addr.ip());
                    log::debug!("IPv4 {:?}", addr.ip().to_ipv4());
                }
            }

            let router = router.clone();
            async move {
                Ok::<_, std::convert::Infallible>(hyper::service::service_fn(move |req| {
                    let router = router.clone();
                    async move { router.handle(req).await }
                }))
            }
        });

    let server = hyper::Server::bind(&host).serve(make_svc);

    let graceful = server.with_graceful_shutdown(shutdown_signal(exit_receiver));

    log::info!("Server now listening on {:?}", host);

    if let Err(e) = graceful.await {
        log::error!("server error: {}", e);
    }

    log::info!("Exiting");
}
