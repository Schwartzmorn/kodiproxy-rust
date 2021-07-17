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
        let request = hyper::Request::builder()
            .method("GET")
            .uri(self.target.to_owned() + "cec/standby")
            .body(hyper::body::Body::empty())
            .unwrap();
        futures::executor::block_on(hyper::Client::new().request(request)).unwrap();
        Ok(())
    }
}
