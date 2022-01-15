pub struct FileClient {
    scheme: String,
    authority: String,
}

impl FileClient {
    pub async fn get_versions(
        &self,
        parts: &http::request::Parts,
    ) -> Result<Vec<files::log::FileLogEntry>, router::RouterError> {
        let path = String::from("/file-versions/") + get_path_from_uri(&parts.uri)?;

        let uri = hyper::Uri::builder()
            .scheme(self.scheme.as_str())
            .authority(self.authority.as_str())
            .path_and_query(path)
            .build()
            .unwrap();

        // TODO handle headers
        let request = hyper::Request::builder()
            .method(&hyper::Method::GET)
            .uri(uri.to_owned())
            .version(parts.version)
            .body(hyper::Body::empty())
            .unwrap();

        let response = hyper::Client::new().request(request).await.map_err(|err| {
            log::warn!("Encountered retrieving versions {}: {:?}", uri, err);
            router::RouterError::HandlerError(500, String::from("Error while retrieving versions"))
        })?;

        let (_, body) = response.into_parts();

        let response = hyper::body::to_bytes(body)
            .await
            .map(|b| b.to_vec())
            .map_err(|e| router::HandlerError(500, format!("Could not decode versions: {}", e)))?;
        let response = String::from_utf8(response)
            .map_err(|e| router::HandlerError(500, format!("Could not decode versions: {}", e)))?;
        serde_json::from_str(&response)
            .map_err(|e| router::HandlerError(500, format!("Could not decode versions: {}", e)))
    }

    pub async fn forward(
        &self,
        parts: &http::request::Parts,
        body: &Vec<u8>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let path_and_query = parts
            .uri
            .path_and_query()
            .map_or_else(|| parts.uri.path().to_owned(), |pq| pq.as_str().to_owned());
        let uri = hyper::Uri::builder()
            .scheme(self.scheme.as_str())
            .authority(self.authority.as_str())
            .path_and_query(path_and_query)
            .build()
            .unwrap();
        let method = parts.method.to_owned();
        let version = parts.version;

        let body = hyper::Body::from(body.clone());

        let mut builder = hyper::Request::builder()
            .method(method)
            .uri(uri.to_owned())
            .version(version);

        for (header_name, header_value) in &parts.headers {
            builder = builder.header(header_name, header_value);
        }

        let request = builder.body(body).unwrap();

        return hyper::Client::new().request(request).await.map_err(|err| {
            log::warn!(
                "Encountered error while forwarding request {}: {:?}",
                uri,
                err
            );
            router::RouterError::HandlerError(500, String::from("Error while forwarding the query"))
        });
    }
}

fn get_path_from_uri(uri: &http::Uri) -> Result<&str, router::RouterError> {
    lazy_static::lazy_static! {
        static ref URI_REGEX: regex::Regex = regex::Regex::new(r"^/files/(.+)").unwrap();
    }
    let matches = URI_REGEX.captures(uri.path());
    match matches {
        Some(matches) => Ok(matches.get(1).unwrap().as_str()),
        None => Err(router::InvalidRequest(String::from("Invalid url"))),
    }
}
