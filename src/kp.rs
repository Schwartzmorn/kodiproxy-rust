mod avreceiver;
mod cec;
pub mod configuration;
mod dbus;
mod files;
mod handlers;
mod router;

fn register_handlers_kp(
    configuration: &configuration::ProxyConfiguration,
    router: &mut router::Router,
) {
    let avreceiver = avreceiver::get_avreceiver(&configuration.receiver);
    let cec_interface = cec::get_cec_connection(&configuration.cec);

    router
        .add_handler(handlers::jsonrpc::get_jrpc_handler(
            &configuration.jrpc,
            avreceiver.clone(),
            cec_interface.clone(),
        ))
        .add_handlers(files::get_file_handlers(&configuration.file))
        .add_handlers(handlers::cec::get_cec_handlers(cec_interface.clone()));
}

pub async fn serve_kp(
    configuration: &configuration::ProxyConfiguration,
    exit_channel: Option<futures::channel::oneshot::Receiver<()>>,
) {
    let addr = router::get_socketaddr(&configuration.server);

    let connection = crate::dbus::AvahiConnection::new(addr.port());

    match &connection {
        Ok(_) => (),
        Err(e) => log::warn!("Failed to register server in Avahi: {:?}", e),
    }

    router::serve(&configuration.server, exit_channel, |router| {
        register_handlers_kp(configuration, router)
    })
    .await;
}
