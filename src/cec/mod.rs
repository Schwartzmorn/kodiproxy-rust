pub use cec::CECInterface;
pub use enums::CECLogicalAddress;

mod cec;
mod cec_fake;
mod enums;
mod functions;
mod structs;

#[cfg(test)]
pub use self::cec::MockCECInterface;

pub fn get_cec_connection(
    configuration: &crate::configuration::CECConfiguration,
) -> std::sync::Arc<std::sync::Mutex<dyn cec::CECInterface>> {
    if let Some(target) = &configuration.fake_target {
        std::sync::Arc::new(std::sync::Mutex::new(cec_fake::CECFakeInterface {
            target: target.to_owned(),
        }))
    } else {
        let configuration = cec::LibcecConfigurationBuilder::new()
            .with_client_version(&configuration.cec_version)
            .build()
            .expect("Invalid CEC configuration");
        std::sync::Arc::new(std::sync::Mutex::new(cec::CECConnection::new(
            configuration,
        )))
    }
}
