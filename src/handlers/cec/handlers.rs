use std::str::FromStr;

pub struct CECPowerOn {
    pub connection: std::sync::Arc<std::sync::Mutex<dyn crate::cec::CECInterface>>,
    pub matcher: Box<dyn crate::router::matcher::Matcher>,
}

pub struct CECStandby {
    pub connection: std::sync::Arc<std::sync::Mutex<dyn crate::cec::CECInterface>>,
    pub matcher: Box<dyn crate::router::matcher::Matcher>,
}

// TODO reduce copy paste
#[async_trait::async_trait]
impl crate::router::Handler for CECPowerOn {
    fn get_matcher(&self) -> &Box<dyn crate::router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        let address = form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes())
            .find(|(param, _)| param == "device")
            .map(|(_, value)| crate::cec::CECLogicalAddress::from_str(&value))
            .unwrap_or(Ok(crate::cec::CECLogicalAddress::Broadcast))
            .map_err(|_| {
                crate::router::RouterError::InvalidRequest(String::from("Invalid device parameter"))
            })?;

        self.connection
            .lock()
            .map_err(|_| {
                crate::router::RouterError::HandlerError(
                    500,
                    String::from("Failed to acquire lock on CEC connection"),
                )
            })?
            .power_on(address)
            .map_err(|e| {
                crate::router::RouterError::HandlerError(
                    500,
                    format!("Failed to turn on device: {:?}", e),
                )
            })?;

        Ok(hyper::Response::builder()
            .status(204)
            .body(hyper::Body::empty())
            .unwrap())
    }
}

#[async_trait::async_trait]
impl crate::router::Handler for CECStandby {
    fn get_matcher(&self) -> &Box<dyn crate::router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        let address = form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes())
            .find(|(param, _)| param == "device")
            .map(|(_, value)| crate::cec::CECLogicalAddress::from_str(&value))
            .unwrap_or(Ok(crate::cec::CECLogicalAddress::Broadcast))
            .map_err(|_| {
                crate::router::RouterError::InvalidRequest(String::from("Invalid device parameter"))
            })?;

        self.connection
            .lock()
            .map_err(|_| {
                crate::router::RouterError::HandlerError(
                    500,
                    String::from("Failed to acquire lock on CEC connection"),
                )
            })?
            .standby(address)
            .map_err(|e| {
                crate::router::RouterError::HandlerError(
                    500,
                    format!("Failed put device in standby: {:?}", e),
                )
            })?;

        Ok(hyper::Response::builder()
            .status(204)
            .body(hyper::Body::empty())
            .unwrap())
    }
}
