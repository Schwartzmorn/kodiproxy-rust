/// Handler that takes care of GET requests
pub struct GetFileHandler {
    //pub file_client: std::sync::Arc<crate::file_client::FileClient>,
    pub file_repo: std::sync::Arc<files::FileRepository>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

fn get_path_from_uri(uri: &http::Uri) -> &str {
    &uri.path()[7..]
}

#[async_trait::async_trait]
impl router::Handler for GetFileHandler {
    fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let (parts, body) = request.into_parts();

        let bytes = hyper::body::to_bytes(body).await.unwrap().to_vec(); // TODO err

        // TODO forward
        // if we got response
        //   if successful:
        //      check sha and update local if necessary
        //   if 404:
        //      delete local if necessary
        //   reply same stuff
        // if we did not get response
        //   return local file

        let repo = self
            .file_repo
            .get_single_file_repo(get_path_from_uri(&parts.uri), false)?;

        let filename = repo.get_filename()?;

        let data = repo.get_current_version()?;

        Ok(hyper::Response::builder()
            .status(200)
            .header(
                "content-disposition",
                format!("attachment; filename=\"{}\"", filename),
            )
            .body(hyper::Body::from(data))
            .unwrap())
    }
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn it_pouets() {
        let body = hyper::Body::empty();
        let body = hyper::body::to_bytes(body).await.unwrap().to_vec();

        println!("Body {:?}", body);
    }
}
