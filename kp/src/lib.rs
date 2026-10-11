mod avreceiver;
mod cec;
pub mod configuration;
mod dbus;
mod handlers;

use std::str::FromStr;

pub(crate) fn reqwest_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to build HTTP client")
}

pub async fn serve_kp(
    configuration: &configuration::ProxyConfiguration,
    exit_channel: Option<futures::channel::oneshot::Receiver<()>>,
) {
    let addr = std::net::SocketAddr::from_str(configuration.server.host.as_str())
        .expect("Incorrect host in server configuration");

    let connection = crate::dbus::AvahiConnection::new(addr.port());

    match &connection {
        Ok(_) => (),
        Err(e) => log::warn!("Failed to register server in Avahi: {:?}", e),
    }

    let avreceiver = avreceiver::get_avreceiver(&configuration.receiver);
    let cec_interface = cec::get_cec_connection(&configuration.cec);

    let (files_router, history_router) = files::get_routers(&configuration.file.root_path);

    let router = axum::Router::new()
        .nest(
            "/avreceiver",
            handlers::avreceiver::get_router(avreceiver.clone()),
        )
        .nest("/cec", handlers::cec::get_router(cec_interface.clone()))
        .nest(
            "/vtuner",
            handlers::vtuner::get_router(&configuration.vtuner),
        )
        .nest("/files", files_router)
        .nest("/file-versions", history_router)
        .merge(handlers::jsonrpc::get_router(
            &configuration.jrpc,
            avreceiver,
            cec_interface,
        ));

    router::serve(addr, exit_channel, router).await;
}
