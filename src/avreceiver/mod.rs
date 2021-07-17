pub use self::avreceiver::AVReceiverInterface;

mod avreceiver;

#[cfg(test)]
pub use self::avreceiver::MockAVReceiver;

pub fn get_avreceiver(
    configuration: &crate::configuration::AVReceiverConfiguration,
) -> std::sync::Arc<dyn AVReceiverInterface> {
    std::sync::Arc::new(
        avreceiver::AVReceiver::builder()
            .with_url(configuration.target.to_owned())
            .with_desired_input(configuration.desired_input.to_owned())
            .build(),
    )
}
