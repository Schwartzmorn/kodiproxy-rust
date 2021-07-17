pub struct JRPCGetSystemProperties {}

pub struct JRPCShutdown {
    avreceiver: std::sync::Arc<dyn crate::avreceiver::AVReceiverInterface>,
    cec_interface: std::sync::Arc<std::sync::Mutex<dyn crate::cec::CECInterface>>,
}

impl JRPCGetSystemProperties {
    pub fn new() -> Box<dyn crate::handlers::jsonrpc::JsonrpcOverloader> {
        Box::new(JRPCGetSystemProperties {})
    }
}

impl JRPCShutdown {
    pub fn new(
        avreceiver: std::sync::Arc<dyn crate::avreceiver::AVReceiverInterface>,
        cec_interface: std::sync::Arc<std::sync::Mutex<dyn crate::cec::CECInterface>>,
    ) -> Box<dyn crate::handlers::jsonrpc::JsonrpcOverloader> {
        Box::new(JRPCShutdown {
            avreceiver,
            cec_interface,
        })
    }
}

#[async_trait::async_trait]
impl crate::handlers::jsonrpc::JsonrpcOverloader for JRPCGetSystemProperties {
    async fn handle(
        &self,
        _parts: hyper::http::request::Parts,
        json_request: super::jsonrpc::JRPCQuery,
        _handler: &super::jsonrpc::JsonrpcHandler,
    ) -> Result<super::jsonrpc::JRPCResponse, crate::router::RouterError> {
        if let Some(serde_json::Value::Object(params)) = json_request.params() {
            if let Some(serde_json::Value::Array(properties)) = params.get("properties") {
                let mut result = serde_json::Map::<String, serde_json::Value>::new();
                for value in properties {
                    match value {
                        serde_json::Value::String(property) => {
                            result.insert(
                                property.to_owned(),
                                serde_json::Value::from(property == "canreboot"),
                            );
                        }
                        _ => (),
                    }
                }
                return Ok(crate::handlers::jsonrpc::JRPCResponse::new(
                    Some(serde_json::Value::Object(result)),
                    json_request.id(),
                ));
            }
        }
        Err(crate::router::RouterError::InvalidRequest(String::from(
            "Invalid properties parameter",
        )))
    }
}

#[async_trait::async_trait]
impl crate::handlers::jsonrpc::JsonrpcOverloader for JRPCShutdown {
    async fn handle(
        &self,
        _parts: hyper::http::request::Parts,
        json_request: super::jsonrpc::JRPCQuery,
        _handler: &super::jsonrpc::JsonrpcHandler,
    ) -> Result<super::jsonrpc::JRPCResponse, crate::router::RouterError> {
        let interface = self.cec_interface.clone();
        let async_cec = move || async move {
            let res = interface
                .lock()
                .map(|mut e| e.standby(crate::cec::CECLogicalAddress::TV));
            if let Ok(Ok(())) = res {
                Ok(())
            } else {
                Err(crate::router::RouterError::HandlerError(
                    500,
                    format!("Failed to switch off CEC"),
                ))
            }
        };
        let (_av_power, cec_status) = futures::join!(self.avreceiver.set_power(false), async_cec());
        cec_status?;
        return Ok(crate::handlers::jsonrpc::JRPCResponse::new(
            None,
            json_request.id(),
        ));
    }
}

#[cfg(test)]
mod test {

    #[rstest::fixture]
    fn parts() -> http::request::Parts {
        let (parts, _) = hyper::Request::builder()
            .uri("https://localhost:8080/jsonrpc")
            .body(hyper::Body::empty())
            .unwrap()
            .into_parts();
        parts
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn it_responds_to_system_properties(parts: http::request::Parts) {
        let jrpc_handler = crate::handlers::jsonrpc::JsonrpcHandler::builder().build();

        let jrpc = super::JRPCGetSystemProperties::new();

        let request = crate::handlers::jsonrpc::JRPCQuery::new(
            String::from("System.GetProperties"),
            Some(serde_json::json!({
                "properties": ["canshutdown", "cansuspend", "canhibernate", "canreboot"]
            })),
            Some(42),
        );

        let result = jrpc
            .handle(parts, request, jrpc_handler.as_ref())
            .await
            .unwrap();

        let result = result.result().to_owned().unwrap();
        assert_eq!(
            serde_json::json!({
                "canshutdown": false,
                "cansuspend": false,
                "canhibernate": false,
                "canreboot": true
            }),
            result
        );
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn it_shuts_down_the_receiver_and_the_tv(parts: http::request::Parts) {
        let jrpc_handler = crate::handlers::jsonrpc::JsonrpcHandler::builder().build();

        let mut mock_receiver = crate::avreceiver::MockAVReceiver::new();
        mock_receiver
            .expect_set_power()
            .with(mockall::predicate::eq(false))
            .times(1)
            .returning(|_| false);
        let mock_receiver = std::sync::Arc::new(mock_receiver);

        let mut mock_cec = crate::cec::MockCECInterface::new();
        mock_cec
            .expect_standby()
            .with(mockall::predicate::eq(crate::cec::CECLogicalAddress::TV))
            .times(1)
            .returning(|_| Ok(()));
        let mock_cec = std::sync::Arc::new(std::sync::Mutex::new(mock_cec));

        let jrpc = super::JRPCShutdown::new(mock_receiver, mock_cec);

        let request = crate::handlers::jsonrpc::JRPCQuery::new(
            String::from("Application.Quit"),
            None,
            Some(42),
        );

        jrpc.handle(parts, request, jrpc_handler.as_ref())
            .await
            .unwrap();
    }
}
