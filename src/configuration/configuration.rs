#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CECConfiguration {
    #[serde(rename = "cecVersion", default = "cec_default_version")]
    pub cec_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fakeTarget", default)]
    pub fake_target: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct FileConfiguration {
    #[serde(rename = "rootPath", default = "file_default_root_path")]
    pub root_path: std::path::PathBuf,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct JRPCConfiguration {
    #[serde(default = "jrpc_default_target")]
    pub target: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LoggingConfiguration {
    #[serde(default = "logging_default_enabled")]
    pub enabled: bool,
    #[serde(default = "logging_default_level")]
    #[serde(deserialize_with = "deserialize_level")]
    #[serde(serialize_with = "serialize_level")]
    pub level: log::LevelFilter,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct AVReceiverConfiguration {
    #[serde(rename = "desiredInput", default = "av_default_input")]
    pub desired_input: String,
    #[serde(default = "av_default_target")]
    pub target: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ServerConfiguration {
    #[serde(default = "server_default_host")]
    pub host: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ProxyConfiguration {
    #[serde(default)]
    pub cec: CECConfiguration,
    #[serde(default)]
    pub file: FileConfiguration,
    #[serde(default)]
    pub jrpc: JRPCConfiguration,
    #[serde(default)]
    pub logging: LoggingConfiguration,
    #[serde(default)]
    pub receiver: AVReceiverConfiguration,
    #[serde(default)]
    pub server: ServerConfiguration,
}

impl std::default::Default for CECConfiguration {
    fn default() -> Self {
        CECConfiguration {
            cec_version: cec_default_version(),
            fake_target: None,
        }
    }
}

impl std::default::Default for FileConfiguration {
    fn default() -> Self {
        FileConfiguration {
            root_path: file_default_root_path(),
        }
    }
}

impl std::default::Default for JRPCConfiguration {
    fn default() -> Self {
        JRPCConfiguration {
            target: jrpc_default_target(),
        }
    }
}

impl std::default::Default for LoggingConfiguration {
    fn default() -> Self {
        LoggingConfiguration {
            enabled: logging_default_enabled(),
            level: logging_default_level(),
            path: None,
        }
    }
}

impl std::default::Default for AVReceiverConfiguration {
    fn default() -> Self {
        AVReceiverConfiguration {
            desired_input: av_default_input(),
            target: av_default_target(),
        }
    }
}

impl std::default::Default for ServerConfiguration {
    fn default() -> Self {
        ServerConfiguration {
            host: server_default_host(),
        }
    }
}
fn cec_default_version() -> String {
    String::from("4.0.4")
}

fn file_default_root_path() -> std::path::PathBuf {
    std::path::PathBuf::from("test/path")
}

fn jrpc_default_target() -> String {
    String::from("http://localhost:8081/jsonrpc")
}

fn logging_default_enabled() -> bool {
    true
}

fn logging_default_level() -> log::LevelFilter {
    log::LevelFilter::Warn
}

fn av_default_input() -> String {
    String::from("AUXB")
}

fn av_default_target() -> String {
    String::from("http://192.168.2.40")
}

fn server_default_host() -> String {
    String::from("127.0.0.1:8079")
}

fn deserialize_level<'de, D>(deserializer: D) -> Result<log::LevelFilter, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    match s.to_uppercase().as_str() {
        "OFF" => Ok(log::LevelFilter::Off),
        "ERROR" => Ok(log::LevelFilter::Error),
        "WARN" => Ok(log::LevelFilter::Warn),
        "INFO" => Ok(log::LevelFilter::Info),
        "DEBUG" => Ok(log::LevelFilter::Debug),
        "TRACE" => Ok(log::LevelFilter::Trace),
        _ => Err(serde::de::Error::custom(format!(
            "Invalid log level: {}",
            s
        ))),
    }
}

pub fn serialize_level<S>(level: &log::LevelFilter, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let level = format!("{}", level).to_uppercase();
    s.serialize_str(level.as_str())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_has_a_default_configuration() {
        let json = r#"{}"#;
        let _configuration: ProxyConfiguration =
            serde_json::from_str(json).expect("Could not build a default configuration");
    }

    #[test]
    fn it_decodes_logging() {
        for (json_level, expected_level) in
            [("ERROR", log::Level::Error), ("INFO", log::Level::Info)]
        {
            let json = format!(r#"{{"enabled":true,"level":"{}"}}"#, json_level);
            let de_json =
                serde_json::from_str::<super::LoggingConfiguration>(json.as_str()).unwrap();

            assert_eq!(expected_level, de_json.level);

            let ser_json = serde_json::to_string(&de_json).unwrap();

            assert_eq!(json, ser_json);
        }
    }
}
