pub struct CECFakeInterface {
    pub target: String,
}

/// Fake implementation for integration testing
impl super::CECInterface for CECFakeInterface {
    fn power_on(
        &mut self,
        cec_logical_address: super::CECLogicalAddress,
    ) -> Result<(), super::enums::CECError> {
        log::info!(
            "Received power on request for device {:?}",
            cec_logical_address
        );
        Ok(())
    }

    fn standby(
        &mut self,
        cec_logical_address: super::CECLogicalAddress,
    ) -> Result<(), super::enums::CECError> {
        log::info!(
            "Received stand by request for device {:?}",
            cec_logical_address
        );
        futures::executor::block_on(
            crate::reqwest_client()
                .get(self.target.to_owned() + "cec/standby")
                .send(),
        )
        .unwrap();
        Ok(())
    }
}
