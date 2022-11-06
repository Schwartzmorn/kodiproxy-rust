pub struct AVReceiverVolumeHandler {
    pub receiver: std::sync::Arc<dyn crate::avreceiver::AVReceiverInterface>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

pub struct AVReceiverPowerHandler {
    pub receiver: std::sync::Arc<dyn crate::avreceiver::AVReceiverInterface>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

#[async_trait::async_trait]
impl router::Handler for AVReceiverVolumeHandler {
    fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let (volume, is_mute) = self.handle_volume_request(request.uri()).await?;

        let body = serde_json::json!({
            "data": {
                "volume": volume,
                "mute": is_mute
            }
        })
        .to_string();

        Ok(hyper::Response::builder()
            .status(200)
            .body(hyper::Body::from(body))
            .unwrap())
    }
}

impl AVReceiverVolumeHandler {
    async fn handle_volume_request(
        &self,
        uri: &http::uri::Uri,
    ) -> Result<(i16, bool), router::RouterError> {
        let mut query: std::collections::HashMap<std::borrow::Cow<str>, std::borrow::Cow<str>> =
            form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()).collect();

        let mute = query.remove("mute");
        let volume = query.remove("volume");

        if !query.is_empty() {
            return Err(router::InvalidRequest(String::from(
                "Accepted parameters are 'mute', 'volume'",
            )));
        }

        if let Some(mute) = mute {
            self.mute(mute.to_lowercase()).await?;
        }
        if let Some(volume) = volume {
            self.set_volume(volume.to_lowercase()).await?;
        }

        return Ok(self.receiver.get_volume().await);
    }

    async fn mute(&self, mute: String) -> Result<(), router::RouterError> {
        if mute != "true" && mute != "false" {
            return Err(router::InvalidRequest(String::from(
                "Accepted values for mute are 'true', 'false'",
            )));
        }
        let mute = mute == "true";

        self.receiver.set_mute(mute).await;
        Ok(())
    }

    async fn set_volume(&self, volume: String) -> Result<(), router::RouterError> {
        if volume == "increment" || volume == "decrement" {
            self.receiver.increment_volume(volume == "increment").await;
            return Ok(());
        }

        let volume = volume.parse::<i16>().map_err(|_| {
            router::InvalidRequest(String::from(
                "Accepted values for volume are 0 - 100, 'increment', 'decrement'",
            ))
        })?;
        self.receiver.set_volume(volume).await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl router::Handler for AVReceiverPowerHandler {
    fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let mut query: std::collections::HashMap<std::borrow::Cow<str>, std::borrow::Cow<str>> =
            form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes()).collect();

        let power = query.remove("power");

        if !query.is_empty() {
            return Err(router::InvalidRequest(String::from(
                "Accepted parameters are 'power'",
            )));
        }

        if let Some(power) = power {
            let power = power.to_lowercase();
            if power != "on" && power != "off" {
                return Err(router::InvalidRequest(String::from(
                    "Accepted values for power are 'on', 'off'",
                )));
            }
            self.receiver.set_power(power == "on").await;
        }

        let power = self.receiver.is_powered_on().await;

        let body = serde_json::json!({
            "data": {
                "power": power
            }
        })
        .to_string();

        Ok(hyper::Response::builder()
            .status(200)
            .body(hyper::Body::from(body))
            .unwrap())
    }
}

#[cfg(test)]
mod tests {
    use router::Handler;
    use test_log::test;

    #[test(tokio::test)]
    async fn it_allows_setting_volume() {
        let mut receiver_mock = crate::avreceiver::MockAVReceiver::new();

        receiver_mock
            .expect_set_volume()
            .with(mockall::predicate::eq(25))
            .times(1)
            .returning(|_| 20);

        receiver_mock
            .expect_increment_volume()
            .with(mockall::predicate::eq(true))
            .times(1)
            .returning(|_| 20);

        receiver_mock
            .expect_get_volume()
            .times(2)
            .returning(|| (25, false));

        let receiver_mock = std::sync::Arc::new(receiver_mock);
        let handler = super::AVReceiverVolumeHandler {
            receiver: receiver_mock.clone(),
            matcher: crate::handlers::avreceiver::get_matcher("volume"),
        };

        let request = hyper::Request::builder()
            .uri("/avreceiver/volume?volume=25")
            .method("GET")
            .body(hyper::Body::empty())
            .unwrap();

        handler.handle(request).await.unwrap();

        let request = hyper::Request::builder()
            .uri("/avreceiver/volume?volume=increment")
            .method("GET")
            .body(hyper::Body::empty())
            .unwrap();

        handler.handle(request).await.unwrap();
    }

    #[test(tokio::test)]
    async fn it_allows_powering() {
        let mut receiver_mock = crate::avreceiver::MockAVReceiver::new();

        receiver_mock
            .expect_set_power()
            .with(mockall::predicate::eq(true))
            .times(1)
            .returning(|_| true);

        receiver_mock
            .expect_is_powered_on()
            .times(1)
            .returning(|| true);

        let receiver_mock = std::sync::Arc::new(receiver_mock);
        let handler = super::AVReceiverPowerHandler {
            receiver: receiver_mock.clone(),
            matcher: crate::handlers::avreceiver::get_matcher("power"),
        };

        let request = hyper::Request::builder()
            .uri("/avreceiver/power?power=on")
            .method("GET")
            .body(hyper::Body::empty())
            .unwrap();

        handler.handle(request).await.unwrap();
    }
}
