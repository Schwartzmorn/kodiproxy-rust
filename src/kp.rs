use std::str::FromStr;

use futures::FutureExt;

pub mod avreceiver;
pub mod cec;
pub mod configuration;
pub mod dbus;
pub mod handlers;
pub mod router;

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

fn get_socketaddr(configuration: &configuration::ServerConfiguration) -> std::net::SocketAddr {
    let res = std::net::SocketAddr::from_str(configuration.host.as_str())
        .expect("Incorrect host in server configuration");

    res
}

fn setup_router(
    configuration: &configuration::ProxyConfiguration,
) -> std::sync::Arc<router::Router> {
    let avreceiver = avreceiver::get_avreceiver(&configuration.receiver);
    let cec_interface = cec::get_cec_connection(&configuration.cec);

    let router = router::Router::new()
        .add_handler(handlers::jsonrpc::get_jrpc_handler(
            &configuration.jrpc,
            avreceiver.clone(),
            cec_interface.clone(),
        ))
        .add_handlers(handlers::files::get_file_handlers(&configuration.file))
        .add_handlers(handlers::cec::get_cec_handlers(cec_interface.clone()));

    std::sync::Arc::new(router)
}

pub async fn serve(
    configuration: &configuration::ProxyConfiguration,
    exit_channel: Option<futures::channel::oneshot::Receiver<()>>,
) {
    let addr = get_socketaddr(&configuration.server);

    let connection = dbus::AvahiConnection::new(addr.port());

    match &connection {
        Ok(_) => (),
        Err(e) => log::warn!("Failed to register server in Avahi: {:?}", e),
    }

    let router = setup_router(&configuration);

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

    let (_sender, mut receiver) = futures::channel::oneshot::channel::<()>();
    if let Some(rcv) = exit_channel {
        receiver = rcv;
    }

    let graceful = server.with_graceful_shutdown(shutdown_signal(receiver));

    log::info!("Server now listening on {}", configuration.server.host);

    if let Err(e) = graceful.await {
        log::error!("server error: {}", e);
    }

    log::info!("Exiting");
}
