fn reqwest_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to build HTTP client")
}

pub struct FileClient {
    scheme: String,
    authority: String,
}

pub struct File {
    pub sync_information: Option<crate::SyncInformation>,
    pub file: Vec<u8>,
}

pub struct MoveInformation {
    pub from_sync_information: Option<crate::SyncInformation>,
    pub to_sync_information: Option<crate::SyncInformation>,
}

impl FileClient {
    pub async fn get(&self, file_path: &str, file_name: &str) -> Result<File, router::RouterError> {
        let response = reqwest_client()
            .get(self.get_url(file_path, file_name))
            .version(http::Version::HTTP_11)
            .send()
            .await
            .map_err(|e| {
                crate::map_error(
                    &e,
                    format!("Error while retrieving file {}/{}", file_path, file_name,),
                    500,
                )
            })?;

        match response.status() {
            http::StatusCode::OK => {}
            http::StatusCode::NOT_FOUND => return Err(router::RouterError::NotFound),
            code => {
                return Err(router::RouterError::HandlerError(
                    500,
                    format!("Error while retrieving file: received code {}", code),
                ));
            }
        }

        let sync_information = get_sync_information(response.headers());

        let file = response
            .bytes()
            .await
            .map_err(|e| crate::map_error(&e, "Error while decoding file", 500))?
            .to_vec();

        Ok(File {
            sync_information,
            file,
        })
    }

    pub async fn save(
        &self,
        file_path: &str,
        file_name: &str,
        file: Vec<u8>,
    ) -> Result<Option<crate::SyncInformation>, router::RouterError> {
        let response = reqwest_client()
            .put(self.get_url(file_path, file_name))
            .version(http::Version::HTTP_11)
            .body(file)
            .send()
            .await
            .map_err(|e| {
                crate::map_error(
                    &e,
                    format!("Error while saving file {}/{}", file_path, file_name,),
                    500,
                )
            })?;

        let sync_information = get_sync_information(response.headers());

        match response.status() {
            http::StatusCode::OK | http::StatusCode::CREATED => Ok(sync_information),
            status => Err(router::RouterError::HandlerError(
                500,
                format!("Unexpected status code from repository: {}", status),
            )),
        }
    }

    pub async fn delete(
        &self,
        file_path: &str,
        file_name: &str,
    ) -> Result<Option<crate::SyncInformation>, router::RouterError> {
        let response = reqwest_client()
            .delete(self.get_url(file_path, file_name))
            .version(http::Version::HTTP_11)
            .send()
            .await
            .map_err(|e| {
                crate::map_error(
                    &e,
                    format!("Error while deleting file {}/{}", file_path, file_name,),
                    500,
                )
            })?;

        let sync_information = get_sync_information(response.headers());

        match response.status() {
            http::StatusCode::OK | http::StatusCode::NO_CONTENT => Ok(sync_information),
            status => Err(router::RouterError::HandlerError(
                500,
                format!("Unexpected status code from repository: {}", status),
            )),
        }
    }

    pub async fn move_to(
        &self,
        file_path_from: &str,
        file_name_from: &str,
        file_path_to: &str,
        file_name_to: &str,
    ) -> Result<MoveInformation, router::RouterError> {
        let response = reqwest_client()
            .request(
                http::Method::from_bytes(b"MOVE").unwrap(),
                self.get_url(file_path_from, file_name_from),
            )
            .version(http::Version::HTTP_11)
            .header(
                "destination",
                format!("/files/{}/{}", file_path_to, file_name_to),
            )
            .send()
            .await
            .map_err(|e| {
                crate::map_error(
                    &e,
                    format!(
                        "Error while deleting file {}/{}",
                        file_path_from, file_name_from,
                    ),
                    500,
                )
            })?;

        let from_sync_information = get_sync_information(response.headers());

        let response_to = reqwest_client()
            .head(self.get_url(file_path_to, file_name_to))
            .version(http::Version::HTTP_11)
            .send()
            .await;

        let to_sync_information = match response_to {
            Ok(response) => get_sync_information(response.headers()),
            Err(_) => None,
        };

        match response.status() {
            http::StatusCode::OK | http::StatusCode::NO_CONTENT => Ok(MoveInformation {
                from_sync_information,
                to_sync_information,
            }),
            status => Err(router::RouterError::HandlerError(
                500,
                format!("Unexpected status code from repository: {}", status),
            )),
        }
    }

    fn get_url(&self, file_path: &str, file_name: &str) -> String {
        let path = format!("/files/{}/{}", file_path, file_name);
        format!("{}://{}{}", self.scheme, self.authority, path)
    }
}

// TODO use files crate instead
fn get_sync_information(headers: &http::HeaderMap) -> Option<crate::SyncInformation> {
    lazy_static::lazy_static! {
        static ref ETAG_REGEX: regex::Regex = regex::Regex::new(r#"\s*"(\d+)"\s*"#).unwrap();
    }
    let version = headers
        .get(http::header::ETAG)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            ETAG_REGEX
                .captures(h)
                .and_then(|m| m.get(1).map(|c| c.as_str()))
        })
        .and_then(|s| s.parse().ok())?;
    let timestamp = headers
        .get(http::header::LAST_MODIFIED)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| chrono::DateTime::parse_from_rfc3339(h).ok())
        .map(|ts| ts.with_timezone(&chrono::Utc))?;
    Some(crate::SyncInformation {
        last_synced_version: version,
        last_synced_timestamp: timestamp,
    })
}
