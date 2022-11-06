#[async_trait::async_trait]
pub trait AVReceiverInterface: Sync + Send {
    /// Returns whether the receiver is powered on or not
    async fn is_powered_on(&self) -> bool;

    /// Switches on or off the receiver.
    ///
    /// When switching on, the input will also be changed to the desired one
    ///
    /// The receiver will only be actually switched off if it is currently using the desired input
    async fn set_power(&self, on: bool) -> bool;

    /// Mutes / unmutes the receiver
    async fn set_mute(&self, mute: bool) -> bool;

    /// Increments or decrements volume and returns the resulting volume
    async fn increment_volume(&self, increment: bool) -> i16;

    /// Gives the current volume and mute status of the receiver
    async fn get_volume(&self) -> (i16, bool);

    /// Sets the volume, taking a percentage in input, and returns the resulting volume
    async fn set_volume(&self, volume: i16) -> i16;
}

/// Builder for [AVReceiver](crate::avreceiver::AVReceiver)
pub struct AVReceiverBuilder {
    scheme: String,
    authority: String,
    desired_input: String,
    min_volume: f32,
    max_volume: f32,
}

impl AVReceiverBuilder {
    /// Gives the url of the av receiver (scheme + authority)
    pub fn with_url(mut self, url: String) -> AVReceiverBuilder {
        let (scheme, authority, _) = router::parse_url(&url);

        self.scheme = scheme;
        self.authority = authority;

        self
    }

    /// Gives the input that should be selected when powering on
    pub fn with_desired_input(mut self, input: String) -> AVReceiverBuilder {
        self.desired_input = input;
        self
    }

    /// Gives the minimum and maximum volume possible on the receiver
    #[allow(dead_code)]
    pub fn with_volume_range(mut self, min: f32, max: f32) -> AVReceiverBuilder {
        if max <= min {
            panic!("Invalid volume range")
        }
        self.min_volume = min;
        self.max_volume = max;
        self
    }

    /// Consumes the builder and build the [AVReceiver](crate::avreceiver::AVReceiver)
    pub fn build(self) -> AVReceiver {
        AVReceiver {
            scheme: self.scheme,
            authority: self.authority,
            desired_input: self.desired_input,
            min_volume: self.min_volume,
            max_volume: self.max_volume,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct Value {
    #[serde(rename = "value")]
    value: String,
}

#[derive(Debug, serde::Deserialize)]
struct Item {
    #[serde(rename = "Power")]
    power: Option<Value>,
    #[serde(rename = "InputFuncSelect")]
    input_func_select: Option<Value>,
    #[serde(rename = "MasterVolume")]
    master_volume: Option<Value>,
    #[serde(rename = "Mute")]
    mute: Option<Value>,
}

impl Item {
    pub fn is_powered_on(&self) -> bool {
        self.power
            .as_ref()
            .map(|val| val.value == "ON")
            .unwrap_or(false)
    }

    pub fn get_input(&self) -> String {
        self.input_func_select
            .as_ref()
            .map(|val| String::from(&val.value))
            .unwrap_or(String::from(""))
    }

    pub fn is_muted(&self) -> bool {
        self.mute.as_ref().map(|s| s.value == "on").unwrap_or(false)
    }

    pub fn get_volume_db(&self, receiver: &AVReceiver) -> f32 {
        self.master_volume
            .as_ref()
            .map(|volume| volume.value.parse::<f32>().unwrap_or(receiver.min_volume))
            .unwrap_or(receiver.min_volume)
    }

    pub fn get_volume_percent(&self, receiver: &AVReceiver) -> i16 {
        self.master_volume
            .as_ref()
            .map(|volume| receiver.db_to_percent(&volume.value))
            .unwrap_or(0)
    }
}

static CMD_MUTE: &str = "formiPhoneAppMute.xml?1+Mute";
static CMD_POWER: &str = "formiPhoneAppPower.xml?1+Power";
static CMD_SOURCE: &str = "formiPhoneAppDirect.xml?SI";
static CMD_STATUS: &str = "formMainZone_MainZoneXmlStatus.xml";
static CMD_VOLUME: &str = "formiPhoneAppVolume.xml?1+";

/// Minimal interface to the M-CR510 av receiver needed by the kodi proxy
pub struct AVReceiver {
    scheme: String,
    authority: String,
    desired_input: String,
    min_volume: f32,
    max_volume: f32,
}

impl AVReceiver {
    /// Returns a new [AVReceiverBuilder](crate::avreceiver::AVReceiverBuilder) with default values
    pub fn builder() -> AVReceiverBuilder {
        AVReceiverBuilder {
            authority: String::from("localhost"),
            desired_input: String::from("AUXB"),
            min_volume: -80.0,
            max_volume: -20.0,
            scheme: String::from("http"),
        }
    }

    async fn send_command(&self, cmd: String) -> Result<Item, router::RouterError> {
        self.send_command_inner(cmd, true).await
    }

    async fn send_command_inner(
        &self,
        cmd: String,
        expect_body: bool,
    ) -> Result<Item, router::RouterError> {
        let uri = hyper::Uri::builder()
            .scheme(self.scheme.as_str())
            .authority(self.authority.as_str())
            .path_and_query(format!("{}{}", "/goform/", cmd).as_str())
            .build()
            .unwrap();

        let request = hyper::Request::builder()
            .method(hyper::Method::GET)
            .uri(uri)
            .version(hyper::Version::HTTP_11)
            .body(hyper::body::Body::empty())
            .unwrap();

        let mut response = hyper::Client::new().request(request).await.map_err(|err| {
            AVReceiver::error("Error while querying receiver with command", &cmd, err)
        })?;

        let bytes = hyper::body::to_bytes(response.body_mut())
            .await
            .map_err(|err| {
                AVReceiver::error("Could not read av receiver response to command", &cmd, err)
            })?
            .to_vec();

        let payload = String::from_utf8(bytes).map_err(|err| {
            AVReceiver::error("Received invalid utf8 as response from command", &cmd, err)
        })?;

        if expect_body {
            quick_xml::de::from_str(payload.as_str()).map_err(|err| {
                AVReceiver::error("Could not decode receiver response from command", &cmd, err)
            })
        } else {
            Ok(Item {
                power: None,
                input_func_select: None,
                master_volume: None,
                mute: None,
            })
        }
    }

    async fn get_status(&self) -> Result<Item, router::RouterError> {
        self.send_command(String::from(CMD_STATUS)).await
    }

    async fn set_source(&self) -> bool {
        // no body in the response when setting source
        let _ = self
            .send_command_inner(format!("{}{}", CMD_SOURCE, self.desired_input), false)
            .await;
        match self.get_status().await {
            Ok(res) => res.get_input() == self.desired_input,
            Err(_) => false,
        }
    }

    fn db_to_percent(&self, volume: &String) -> i16 {
        // we receive "--" in case the volume is at its minimum
        let mut volume = volume
            .parse::<f32>()
            .or::<f32>(Ok(self.min_volume))
            .unwrap();

        volume = volume - self.min_volume;
        volume /= self.max_volume - self.min_volume;
        (volume * 100.0).round() as i16
    }

    fn percent_to_db(&self, volume: i16) -> f32 {
        let mut volume = volume as f32 / 100.0;
        volume = volume * (self.max_volume - self.min_volume);
        // even though we want a float, we want it to take an integer value
        (volume + self.min_volume).round()
    }

    fn error<T: std::fmt::Display + 'static>(
        msg: &str,
        cmd: &String,
        err: T,
    ) -> router::RouterError {
        let msg = format!("{} '{}': [{}]", msg, cmd, err);
        log::warn!("{}", msg);
        router::HandlerError(502, msg)
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
impl AVReceiverInterface for AVReceiver {
    async fn is_powered_on(&self) -> bool {
        match self.get_status().await {
            Ok(item) => match item.power {
                Some(power) => power.value == "ON",
                None => false,
            },
            _ => false,
        }
    }

    async fn set_power(&self, on: bool) -> bool {
        let status = self.get_status().await;
        let (is_powered_on, mut is_input_ok) = status
            .map(|s| (s.is_powered_on(), s.get_input() == self.desired_input))
            .unwrap_or((false, false));
        if on {
            if !is_powered_on {
                let _ = self.send_command(format!("{}{}", CMD_POWER, "On")).await;
            }
            while !is_input_ok {
                async_std::task::sleep(std::time::Duration::from_millis(500)).await;
                is_input_ok = self.set_source().await;
            }
            true
        } else {
            if is_input_ok {
                let _ = self
                    .send_command(format!("{}{}", CMD_POWER, "Standby"))
                    .await;
            }
            false
        }
    }

    async fn set_mute(&self, mute: bool) -> bool {
        self.send_command(format!("{}{}", CMD_MUTE, if mute { "On" } else { "Off" }))
            .await
            .map(|res| res.is_muted())
            .unwrap_or(false)
    }

    async fn increment_volume(&self, increment: bool) -> i16 {
        // Setting the volume works better than to use the increment / decrement
        let mut volume = self
            .get_status()
            .await
            .map(|item| item.get_volume_db(&self))
            .unwrap_or(self.min_volume);

        volume = volume + if increment { 1.0 } else { -1.0 };
        volume = volume.clamp(self.min_volume, self.max_volume);

        self.send_command(format!("{}{:.1}", CMD_VOLUME, volume))
            .await
            .map(|item| item.get_volume_percent(&self))
            .unwrap_or(0)
    }

    async fn get_volume(&self) -> (i16, bool) {
        self.get_status()
            .await
            .map(|item| (item.get_volume_percent(&self), item.is_muted()))
            .unwrap_or((0, false))
    }

    async fn set_volume(&self, volume: i16) -> i16 {
        let volume = volume.clamp(0, 100);
        let volume = self.percent_to_db(volume);
        self.send_command(format!("{}{:.1}", CMD_VOLUME, volume))
            .await
            .map(|item| item.get_volume_percent(&self))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::AVReceiverInterface;
    use test_log::test;

    fn get_receiver(mock_server: &wiremock::MockServer) -> super::AVReceiver {
        super::AVReceiver::builder()
            .with_url(mock_server.uri())
            .with_desired_input(String::from("AUXB"))
            .with_volume_range(-80.0, -20.0)
            .build()
    }

    fn get_power_response(is_powered: bool) -> String {
        format!(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<item>
<Power><value>{}</value></Power>
</item>"#,
            if is_powered { "ON" } else { "STANDBY" }
        )
    }

    fn get_volume_response(volume: String, mute: bool) -> String {
        format!(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<item>
<MasterVolume><value>{}</value></MasterVolume>
<Mute><value>{}</value></Mute>
</item>"#,
            volume,
            if mute { "on" } else { "off" }
        )
    }

    fn get_status_body(is_powered: bool, input: &str, volume: f32, mute: bool) -> String {
        format!(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<item>
<Zone><value>MainZone</value></Zone>
<Power><value>{}</value></Power>
<Model><value></value></Model>
<InputFuncSelect><value>{}</value></InputFuncSelect>
<MasterVolume><value>{:.1}</value></MasterVolume>
<Mute><value>{}</value></Mute>
</item>"#,
            if is_powered { "ON" } else { "STANDBY" },
            input,
            volume,
            if mute { "on" } else { "off" }
        )
    }

    #[test(tokio::test)]
    async fn it_gives_correct_status_receiver_off() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_bytes(get_power_response(false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert_eq!(false, receiver.is_powered_on().await);
    }

    #[test(tokio::test)]
    async fn it_gives_correct_min_volume() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_volume_response(String::from("--"), false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert_eq!((0, false), receiver.get_volume().await);
    }

    #[test(tokio::test)]
    async fn it_gives_correct_status_receiver_on() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_status_body(true, "NET", -40.0, true)),
            )
            .expect(2)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert!(receiver.is_powered_on().await);
        assert_eq!((67, true), receiver.get_volume().await);
    }

    #[test_log::test(tokio::test)]
    async fn it_mutes_and_unmutes() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(r"/goform/formiPhoneAppMute.xml"))
            .and(wiremock::matchers::query_param("1 MuteOn", ""))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_volume_response(String::from("-40.0"), true)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/goform/formiPhoneAppMute.xml"))
            .and(wiremock::matchers::query_param("1 MuteOff", ""))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_volume_response(String::from("-40.0"), false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert!(receiver.set_mute(true).await);

        assert!(!receiver.set_mute(false).await);
    }

    use wiremock::Match;

    /// Very buggy implementation of a matcher that makes it possible to change the response to a given matcher
    struct NCallsMatcher {
        n_calls: std::sync::Arc<std::sync::Mutex<i16>>,
        min_calls: i16,
        max_calls: i16,
    }

    impl NCallsMatcher {
        pub fn new(n_calls: i16) -> (NCallsMatcher, NCallsMatcher) {
            let n_calls_mutex = std::sync::Arc::new(std::sync::Mutex::new(0));
            (
                NCallsMatcher {
                    n_calls: n_calls_mutex.clone(),
                    min_calls: 0,
                    max_calls: n_calls,
                },
                NCallsMatcher {
                    n_calls: n_calls_mutex.clone(),
                    min_calls: n_calls,
                    max_calls: std::i16::MAX,
                },
            )
        }
    }

    impl Match for NCallsMatcher {
        fn matches(&self, _request: &wiremock::Request) -> bool {
            let mut n_calls = self.n_calls.lock().unwrap();
            if *n_calls >= self.min_calls && *n_calls < self.max_calls {
                *n_calls += 1;
                true
            } else {
                false
            }
        }
    }

    #[test(tokio::test)]
    async fn it_changes_input_when_switching_on() {
        let mock_server = wiremock::MockServer::start().await;

        let (first_calls, last_calls) = NCallsMatcher::new(3);

        // the first three calls to status return the wrong input source
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .and(first_calls)
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_status_body(false, "NET", -40.0, false)),
            )
            .expect(3)
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .and(last_calls)
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_status_body(false, "AUXB", -40.0, false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/goform/formiPhoneAppPower.xml"))
            .and(wiremock::matchers::query_param("1 PowerOn", ""))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_bytes(get_power_response(true)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        // we had to ask the change of source 3 times
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/goform/formiPhoneAppDirect.xml"))
            .and(wiremock::matchers::query_param("SIAUXB", ""))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .expect(3)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert!(receiver.set_power(true).await);
    }

    #[test(tokio::test)]
    async fn it_switches_off_when_the_input_is_ok() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_status_body(true, "AUXB", -40.0, false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/goform/formiPhoneAppPower.xml"))
            .and(wiremock::matchers::query_param("1 PowerStandby", ""))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_bytes(get_power_response(true)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert!(!receiver.set_power(false).await);
    }

    #[test(tokio::test)]
    async fn it_does_not_switch_off_if_the_input_is_not_ok() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_status_body(true, "NET", -40.0, false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/goform/formiPhoneAppPower.xml"))
            .and(wiremock::matchers::query_param("1 PowerStandby", ""))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_bytes(get_power_response(true)),
            )
            .expect(0)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert!(!receiver.set_power(false).await);
    }

    #[test(tokio::test)]
    async fn it_sets_volume() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/goform/formiPhoneAppVolume.xml"))
            .and(wiremock::matchers::query_param("1 -65.0", ""))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_status_body(true, "AUXB", -50.0, false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert_eq!(50, receiver.set_volume(25).await);
    }

    #[test(tokio::test)]
    async fn it_increments_volume() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_volume_response(String::from("-40.0"), false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/goform/formiPhoneAppVolume.xml"))
            .and(wiremock::matchers::query_param("1 -39.0", ""))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_volume_response(String::from("-35.0"), false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert_eq!(75, receiver.increment_volume(true).await);
    }

    #[test(tokio::test)]
    async fn it_decrements_volume() {
        let mock_server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/goform/formMainZone_MainZoneXmlStatus.xml",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_volume_response(String::from("--"), false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/goform/formiPhoneAppVolume.xml"))
            .and(wiremock::matchers::query_param("1 -80.0", ""))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_bytes(get_volume_response(String::from("-65.0"), false)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let receiver = get_receiver(&mock_server);

        assert_eq!(25, receiver.increment_volume(false).await);
    }
}
