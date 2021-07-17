use self::jsonrpc::{JRPCQuery, JRPCResponse, JsonrpcHandler, JsonrpcOverloader};
use self::poweroverloaders::*;
use self::volumeoverloaders::*;

mod jsonrpc;
mod poweroverloaders;
mod volumeoverloaders;

pub fn get_jrpc_handler(
    configuration: &crate::configuration::JRPCConfiguration,
    avreceiver: std::sync::Arc<dyn crate::avreceiver::AVReceiverInterface>,
    cec_interface: std::sync::Arc<std::sync::Mutex<dyn crate::cec::CECInterface>>,
) -> Box<dyn crate::router::Handler> {
    let mut builder = jsonrpc::JsonrpcHandler::builder()
        .with_url(&configuration.target)
        .add_overloader(
            "Application.SetVolume",
            JRPCSetVolume::new(avreceiver.clone()),
        )
        .add_overloader("Application.SetMute", JRPCSetMute::new(avreceiver.clone()))
        .add_overloader(
            "Application.GetProperties",
            JRPCGetProperties::new(avreceiver.clone()),
        )
        .add_overloader("System.GetProperties", JRPCGetSystemProperties::new());
    for method in [
        "Application.Quit",
        "System.Hibernate",
        "System.Shutdown",
        "System.Suspend",
    ] {
        builder = builder.add_overloader(
            method,
            JRPCShutdown::new(avreceiver.clone(), cec_interface.clone()),
        );
    }
    builder.build()
}
