pub use cec::CECInterface;
pub use enums::CECError;
pub use enums::CECLogicalAddress;

#[allow(clippy::module_inception)]
mod cec;
mod cec_fake;
mod enums;
mod functions;
mod structs;

#[cfg(test)]
pub use self::cec::MockCECInterface;

pub async fn get_cec_connection(
    configuration: &crate::configuration::CECConfiguration,
) -> std::sync::Arc<parking_lot::Mutex<dyn cec::CECInterface>> {
    let fake_target = configuration.fake_target.clone();
    let cec_version = configuration.cec_version.clone();

    tokio::task::spawn_blocking(move || {
        if let Some(target) = fake_target {
            log::info!("Initializing fake CEC client on {}", target);
            std::sync::Arc::new(parking_lot::Mutex::new(cec_fake::CECFakeInterface {
                target,
            })) as std::sync::Arc<parking_lot::Mutex<dyn cec::CECInterface>>
        } else {
            let configuration = cec::LibcecConfigurationBuilder::new()
                .with_client_version(&cec_version)
                .build()
                .expect("Invalid CEC configuration");
            std::sync::Arc::new(parking_lot::Mutex::new(cec::CECConnection::new(
                configuration,
            )))
        }
    })
    .await
    .expect("CEC initialization task failed")
}
