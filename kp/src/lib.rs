#![allow(clippy::double_must_use)]

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
    let axum_router = handlers::jsonrpc::add_routes(
        axum::Router::new(),
        &configuration.jrpc,
        avreceiver.clone(),
        cec_interface.clone(),
    );
    let axum_router = handlers::cec::add_routes(
        handlers::avreceiver::add_routes(axum_router, avreceiver),
        cec_interface.clone(),
    );
    let axum_router = files::add_routes(axum_router, &configuration.file.root_path);

    router::serve(addr, exit_channel, axum_router).await;
}
