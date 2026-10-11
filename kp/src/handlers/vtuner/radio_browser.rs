use serde::Deserialize;
use serde::de::DeserializeOwned;

const DEFAULT_API_BASE_URL: &str = "http://all.api.radio-browser.info/json/";
const USER_AGENT: &str = "vTuner/1.1.0";
const DEFAULT_STATION_LIMIT: &str = "200";

#[derive(Clone)]
pub(super) struct RadioBrowserClient {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct DirectoryEntry {
    pub name: String,
    #[serde(rename = "stationcount")]
    pub station_count: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct RadioBrowserStation {
    pub stationuuid: String,
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub favicon: String,
    #[serde(default)]
    pub tags: String,
    pub countrycode: Option<String>,
    pub codec: Option<String>,
    pub bitrate: Option<u64>,
    pub lastcheckok: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct ResolvedUrl {
    url: String,
}

#[derive(Clone, Copy)]
pub(super) enum DirectoryKind {
    Country,
    Language,
    Genre,
}

impl DirectoryKind {
    fn endpoint(self) -> &'static str {
        match self {
            Self::Country => "countries?hidebroken=true",
            Self::Language => "languages?hidebroken=true",
            Self::Genre => "tags?hidebroken=true",
        }
    }
}

impl Default for RadioBrowserClient {
    fn default() -> Self {
        let base_url = std::env::var("YCAST_RADIOBROWSER_API")
            .unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_owned());
        Self::with_base_url(base_url)
    }
}

impl RadioBrowserClient {
    pub(super) fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: crate::reqwest_client(),
            base_url: base_url.into(),
        }
    }

    pub(super) async fn directories(
        &self,
        kind: DirectoryKind,
    ) -> Result<Vec<DirectoryEntry>, reqwest::Error> {
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            kind.endpoint()
        );
        let entries = self
            .client
            .get(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<DirectoryEntry>>()
            .await?;

        Ok(entries
            .into_iter()
            .filter(|entry| entry.station_count > 5)
            .collect())
    }

    pub(super) async fn stations_by_country(
        &self,
        country: &str,
    ) -> Result<Vec<RadioBrowserStation>, reqwest::Error> {
        self.station_search(vec![
            ("order", "name".to_owned()),
            ("reverse", "false".to_owned()),
            ("countryExact", "true".to_owned()),
            ("country", country.to_owned()),
        ])
        .await
    }

    pub(super) async fn stations_by_language(
        &self,
        language: &str,
    ) -> Result<Vec<RadioBrowserStation>, reqwest::Error> {
        self.station_search(vec![
            ("order", "name".to_owned()),
            ("reverse", "false".to_owned()),
            ("languageExact", "true".to_owned()),
            ("language", language.to_owned()),
        ])
        .await
    }

    pub(super) async fn stations_by_genre(
        &self,
        genre: &str,
    ) -> Result<Vec<RadioBrowserStation>, reqwest::Error> {
        self.station_search(vec![
            ("order", "name".to_owned()),
            ("reverse", "false".to_owned()),
            ("tagExact", "true".to_owned()),
            ("tag", genre.to_owned()),
        ])
        .await
    }

    pub(super) async fn popular_stations(
        &self,
    ) -> Result<Vec<RadioBrowserStation>, reqwest::Error> {
        self.get_json(
            "stations",
            &[
                ("order", "votes".to_owned()),
                ("reverse", "true".to_owned()),
                ("limit", DEFAULT_STATION_LIMIT.to_owned()),
            ],
        )
        .await
    }

    pub(super) async fn search_stations(
        &self,
        name: &str,
    ) -> Result<Vec<RadioBrowserStation>, reqwest::Error> {
        self.station_search(vec![
            ("order", "name".to_owned()),
            ("reverse", "false".to_owned()),
            ("limit", DEFAULT_STATION_LIMIT.to_owned()),
            ("name", name.to_owned()),
        ])
        .await
    }

    async fn station_search(
        &self,
        query: Vec<(&str, String)>,
    ) -> Result<Vec<RadioBrowserStation>, reqwest::Error> {
        self.get_json("stations/search", &query).await
    }

    pub(super) async fn station_by_id(
        &self,
        station_uuid: &str,
    ) -> Result<Option<RadioBrowserStation>, reqwest::Error> {
        let url = self.url_with_path_segment("stations/byuuid", station_uuid);
        self.client
            .get(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<RadioBrowserStation>>()
            .await
            .map(|stations| stations.into_iter().next())
    }

    pub(super) async fn station_icon_url(
        &self,
        station_uuid: &str,
    ) -> Result<Option<String>, reqwest::Error> {
        Ok(self
            .station_by_id(station_uuid)
            .await?
            .map(|station| station.favicon)
            .filter(|favicon| !favicon.is_empty()))
    }

    pub(super) async fn playable_url(
        &self,
        station_uuid: &str,
    ) -> Result<Option<String>, reqwest::Error> {
        let url = self.url_with_path_segment("url", station_uuid);
        self.client
            .get(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<ResolvedUrl>>()
            .await
            .map(|urls| urls.into_iter().next().map(|resolved| resolved.url))
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, reqwest::Error> {
        self.client
            .get(format!("{}/{}", self.base_url.trim_end_matches('/'), path))
            .query(query)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await
    }

    fn url_with_path_segment(&self, path: &str, segment: &str) -> String {
        let mut url = url::Url::parse(&format!("{}/{path}/", self.base_url.trim_end_matches('/')))
            .expect("Radio Browser base URL is valid");
        url.path_segments_mut()
            .expect("Radio Browser URL supports path segments")
            .pop_if_empty()
            .push(segment);
        url.into()
    }
}
