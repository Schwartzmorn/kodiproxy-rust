use crate::avreceiver::AVReceiverInterface;

pub struct JRPCSetVolume {
    receiver: std::sync::Arc<dyn AVReceiverInterface>,
}

pub struct JRPCSetMute {
    receiver: std::sync::Arc<dyn AVReceiverInterface>,
}

pub struct JRPCGetProperties {
    receiver: std::sync::Arc<dyn AVReceiverInterface>,
}

impl JRPCSetVolume {
    pub fn new(
        receiver: std::sync::Arc<dyn AVReceiverInterface>,
    ) -> Box<dyn crate::handlers::jsonrpc::JsonrpcOverloader> {
        Box::new(JRPCSetVolume { receiver })
    }
}

#[async_trait::async_trait]
impl crate::handlers::jsonrpc::JsonrpcOverloader for JRPCSetVolume {
    async fn handle(
        &self,
        _parts: hyper::http::request::Parts,
        json_request: crate::handlers::jsonrpc::JRPCQuery,
        _handler: &crate::handlers::jsonrpc::JsonrpcHandler,
    ) -> Result<crate::handlers::jsonrpc::JRPCResponse, router::RouterError> {
        let volume = json_request.params().and_then(|value| {
            if let serde_json::Value::Object(params) = value {
                params.get("volume")
            } else {
                None
            }
        });
        // TODO improve this when async closures are better handled ?
        let result = match volume {
            Some(volume) => match volume {
                serde_json::Value::Number(volume) => {
                    match volume.as_f64().map(|v| v.max(0.0).min(100.0) as i16) {
                        Some(volume) => Some(self.receiver.set_volume(volume).await),
                        _ => None,
                    }
                }
                serde_json::Value::String(command) => {
                    if command == "increment" {
                        Some(self.receiver.increment_volume(true).await)
                    } else if command == "decrement" {
                        Some(self.receiver.increment_volume(false).await)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        };
        result
            .map(|volume| {
                crate::handlers::jsonrpc::JRPCResponse::new(
                    Some(serde_json::json!(volume)),
                    json_request.id(),
                )
            })
            .ok_or(router::InvalidRequest(String::from(
                "Invalid volume parameter",
            )))
    }
}

impl JRPCSetMute {
    pub fn new(
        receiver: std::sync::Arc<dyn AVReceiverInterface>,
    ) -> Box<dyn crate::handlers::jsonrpc::JsonrpcOverloader> {
        Box::new(JRPCSetMute { receiver })
    }
}

#[async_trait::async_trait]
impl crate::handlers::jsonrpc::JsonrpcOverloader for JRPCSetMute {
    async fn handle(
        &self,
        _parts: hyper::http::request::Parts,
        json_request: crate::handlers::jsonrpc::JRPCQuery,
        _handler: &crate::handlers::jsonrpc::JsonrpcHandler,
    ) -> Result<crate::handlers::jsonrpc::JRPCResponse, router::RouterError> {
        let mute = json_request.params().and_then(|value| {
            if let serde_json::Value::Object(params) = value {
                params.get("mute")
            } else {
                None
            }
        });
        // TODO improve this mess
        let result = match mute {
            Some(mute) => match mute {
                serde_json::Value::Bool(mute) => Some(self.receiver.set_mute(*mute).await),
                serde_json::Value::String(command) => {
                    if command == "toggle" {
                        let (_, mute) = self.receiver.get_volume().await;
                        Some(self.receiver.set_mute(!mute).await)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        };
        result
            .map(|mute| {
                crate::handlers::jsonrpc::JRPCResponse::new(
                    Some(serde_json::json!(mute)),
                    json_request.id(),
                )
            })
            .ok_or(router::InvalidRequest(String::from(
                "Invalid mute parameter",
            )))
    }
}

impl JRPCGetProperties {
    pub fn new(
        receiver: std::sync::Arc<dyn AVReceiverInterface>,
    ) -> Box<dyn crate::handlers::jsonrpc::JsonrpcOverloader> {
        Box::new(JRPCGetProperties { receiver })
    }

    fn is_volume_property(param: &String) -> bool {
        return param == "muted" || param == "volume";
    }

    async fn get_volume_properties(
        &self,
        volume_properties: &Vec<String>,
    ) -> Option<serde_json::Map<String, serde_json::Value>> {
        if volume_properties.is_empty() {
            return None;
        } else {
            let (volume, mute) = self.receiver.get_volume().await;
            let mut res = serde_json::Map::<String, serde_json::Value>::new();
            for param in volume_properties {
                if param == "volume" {
                    res.insert(param.to_owned(), serde_json::Value::from(volume));
                } else if param == "muted" {
                    res.insert(param.to_owned(), serde_json::Value::from(mute));
                }
            }
            return Some(res);
        }
    }

    async fn get_other_properties(
        parts: hyper::http::request::Parts,
        json_request: &crate::handlers::jsonrpc::JRPCQuery,
        handler: &crate::handlers::jsonrpc::JsonrpcHandler,
        properties: Vec<String>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, router::RouterError> {
        if !properties.is_empty() {
            let query = crate::handlers::jsonrpc::JRPCQuery::new(
                json_request.method().to_owned(),
                Some(serde_json::json!({
                    "properties": serde_json::Value::from(properties)
                })),
                json_request.id(),
            );

            let response = handler.forward_jrpc(parts, query).await?;

            match response.result() {
                Some(res) => match res {
                    serde_json::Value::Object(map) => return Ok(map.to_owned()),
                    _ => (),
                },
                None => (),
            }
        }
        Ok(serde_json::Map::<String, serde_json::Value>::new())
    }
}

#[async_trait::async_trait]
impl crate::handlers::jsonrpc::JsonrpcOverloader for JRPCGetProperties {
    async fn handle(
        &self,
        parts: hyper::http::request::Parts,
        json_request: crate::handlers::jsonrpc::JRPCQuery,
        handler: &crate::handlers::jsonrpc::JsonrpcHandler,
    ) -> Result<crate::handlers::jsonrpc::JRPCResponse, router::RouterError> {
        if let Some(serde_json::Value::Object(params)) = json_request.params() {
            if let Some(serde_json::Value::Array(properties)) = params.get("properties") {
                let mut volume_properties = Vec::<String>::new();
                let mut other_properties = Vec::<String>::new();

                for param in properties {
                    match param {
                        serde_json::Value::String(param) => {
                            if JRPCGetProperties::is_volume_property(param) {
                                volume_properties.push(param.to_owned());
                            } else {
                                other_properties.push(param.to_owned());
                            }
                        }
                        _ => (),
                    }
                }

                let (volume_props, other_props) = futures::join!(
                    self.get_volume_properties(&volume_properties),
                    JRPCGetProperties::get_other_properties(
                        parts,
                        &json_request,
                        handler,
                        other_properties
                    )
                );

                let mut other_props = other_props?;

                if let Some(properties) = volume_props {
                    for (key, value) in properties {
                        other_props.insert(key, value);
                    }
                }
                return Ok(crate::handlers::jsonrpc::JRPCResponse::new(
                    Some(serde_json::Value::Object(other_props)),
                    json_request.id(),
                ));
            }
        }
        Err(router::InvalidRequest(String::from(
            "Invalid properties parameter",
        )))
    }
}

#[cfg(test)]
mod tests {
    use test_log::test;
    fn get_request<T: std::fmt::Display>(
        param: &str,
        value: T,
    ) -> crate::handlers::jsonrpc::JRPCQuery {
        serde_json::from_str(
            format!(
                r#"{{"method":"Application.SetVolume", "params": {{"{}": {}}}}}"#,
                param, value
            )
            .as_str(),
        )
        .unwrap()
    }

    fn get_request_str(param: &str, value: &str) -> crate::handlers::jsonrpc::JRPCQuery {
        get_request(param, format!(r#""{}""#, value))
    }

    fn get_parts() -> hyper::http::request::Parts {
        let (parts, _) = hyper::Request::builder()
            .method("POST")
            .uri("https://localhost:8080/jsonrpc")
            .body(hyper::Body::empty())
            .unwrap()
            .into_parts();
        parts
    }

    fn get_jrpc_handler() -> Box<crate::handlers::jsonrpc::JsonrpcHandler> {
        crate::handlers::jsonrpc::JsonrpcHandler::builder().build()
    }

    #[test(tokio::test)]
    async fn it_sets_volume() {
        let mut mock = crate::avreceiver::MockAVReceiver::new();
        mock.expect_set_volume()
            .with(mockall::predicate::eq(25))
            .times(1)
            .returning(|_| 20);
        mock.expect_increment_volume()
            .with(mockall::predicate::eq(true))
            .times(1)
            .returning(|_| 30);
        mock.expect_increment_volume()
            .with(mockall::predicate::eq(false))
            .times(1)
            .returning(|_| 35);
        let mock = std::sync::Arc::new(mock);
        let jrpc = super::JRPCSetVolume::new(mock);

        // set volume
        let parts = get_parts();
        let request = get_request("volume", 25);
        let handler = get_jrpc_handler();

        let res = jrpc.handle(parts, request, handler.as_ref()).await.unwrap();
        let res = res.result().to_owned().unwrap();

        assert_eq!(serde_json::Value::from(20), res);

        // increase volume
        let parts = get_parts();
        let request = get_request_str("volume", "increment");
        let handler = get_jrpc_handler();

        let res = jrpc.handle(parts, request, handler.as_ref()).await.unwrap();
        let res = res.result().to_owned().unwrap();

        assert_eq!(serde_json::Value::from(30), res);

        // decrease volume
        let parts = get_parts();
        let request = get_request_str("volume", "decrement");
        let handler = get_jrpc_handler();

        let res = jrpc.handle(parts, request, handler.as_ref()).await.unwrap();
        let res = res.result().to_owned().unwrap();

        assert_eq!(serde_json::Value::from(35), res);

        // invalid value
        let parts = get_parts();
        let request = get_request_str("invalid", "invalid");
        let handler = get_jrpc_handler();

        let res = jrpc
            .handle(parts, request, handler.as_ref())
            .await
            .unwrap_err();

        assert_eq!(
            router::InvalidRequest(String::from("Invalid volume parameter")),
            res
        );
    }

    #[test(tokio::test)]
    async fn it_mutes() {
        let mut mock = crate::avreceiver::MockAVReceiver::new();
        mock.expect_set_mute()
            .with(mockall::predicate::eq(true))
            .times(1)
            .returning(|_| true);
        mock.expect_set_mute()
            .with(mockall::predicate::eq(false))
            .times(2)
            .returning(|_| false);
        mock.expect_get_volume().times(1).returning(|| (40, true));
        let mock = std::sync::Arc::new(mock);
        let jrpc = super::JRPCSetMute::new(mock);

        // mute
        let parts = get_parts();
        let request = get_request("mute", true);
        let handler = get_jrpc_handler();

        let res = jrpc.handle(parts, request, handler.as_ref()).await.unwrap();
        let res = res.result().to_owned().unwrap();

        assert_eq!(serde_json::Value::from(true), res);

        // unmute
        let parts = get_parts();
        let request = get_request("mute", false);
        let handler = get_jrpc_handler();

        let res = jrpc.handle(parts, request, handler.as_ref()).await.unwrap();
        let res = res.result().to_owned().unwrap();

        assert_eq!(serde_json::Value::from(false), res);

        // unmute
        let parts = get_parts();
        let request = get_request_str("mute", "toggle");
        let handler = get_jrpc_handler();

        let res = jrpc.handle(parts, request, handler.as_ref()).await.unwrap();
        let res = res.result().to_owned().unwrap();

        assert_eq!(serde_json::Value::from(false), res);

        // invalid value
        let parts = get_parts();
        let request = get_request_str("invalid", "invalid");
        let handler = get_jrpc_handler();

        let res = jrpc
            .handle(parts, request, handler.as_ref())
            .await
            .unwrap_err();

        assert_eq!(
            router::InvalidRequest(String::from("Invalid mute parameter")),
            res
        );
    }

    #[test(tokio::test)]
    async fn it_responds_to_properties() {
        let mut mock_receiver = crate::avreceiver::MockAVReceiver::new();
        mock_receiver
            .expect_get_volume()
            .times(1)
            .returning(|| (42, false));
        let mock_receiver = std::sync::Arc::new(mock_receiver);

        let mock_server: wiremock::MockServer = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/jsonrpc"))
            .and(wiremock::matchers::body_string(
                r#"{"jsonrpc":"2.0","method":"Application.GetProperties","params":{"properties":["aProperty1","aProperty2"]},"id":42}"#,
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_bytes(
                    r#"{"result":{"aProperty1":"aValue1","aProperty2":"aValue2"}}"#,
                ),
            )
            .mount(&mock_server)
            .await;

        let jrpc_handler = crate::handlers::jsonrpc::JsonrpcHandler::builder()
            .with_url(&mock_server.uri())
            .build();

        let jrpc = super::JRPCGetProperties::new(mock_receiver);

        let parts = get_parts();

        let request = crate::handlers::jsonrpc::JRPCQuery::new(
            String::from("Application.GetProperties"),
            Some(serde_json::json!({
                "properties": ["muted", "volume", "aProperty1", "aProperty2"]
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
                "muted": false,
                "volume": 42,
                "aProperty1": "aValue1",
                "aProperty2": "aValue2"
            }),
            result
        );
    }
}
