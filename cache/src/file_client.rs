pub struct FileClient {
    scheme: String,
    authority: String,
}

impl FileClient {
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
