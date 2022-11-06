pub struct AVReceiverVolumeHandler {
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
        let query: std::collections::HashMap<std::borrow::Cow<str>, std::borrow::Cow<str>> =
            form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()).collect();

        if let Some(mute) = query.get("mute") {
            self.mute(mute.as_ref()).await?;
        } else if let Some(volume) = query.get("volume") {
            self.set_volume(volume.as_ref()).await?;
        }

        return Ok(self.receiver.get_volume().await);
    }

    async fn mute(&self, mute: &str) -> Result<(), router::RouterError> {
        let mute = mute == "true";
        self.receiver.set_mute(mute).await;
        Ok(())
    }

    async fn set_volume(&self, volume: &str) -> Result<(), router::RouterError> {
        let volume = volume
            .parse::<i16>()
            .map_err(|_| router::InvalidRequest(String::from("Invalid volume")))?;
        self.receiver.set_volume(volume).await;
        Ok(())
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
            .expect_get_volume()
            .times(1)
            .returning(|| (25, true));

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
    }
}
