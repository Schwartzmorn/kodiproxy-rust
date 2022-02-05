/// Handler that takes care of GET requests
pub struct GetFileHandler {
    pub file_client: std::sync::Arc<crate::file_client::FileClient>,
    pub file_repo: std::sync::Arc<files::FileRepository>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

fn get_path_from_uri(uri: &http::Uri) -> &str {
    &uri.path()[7..]
}

//#[async_trait::async_trait]
//impl router::Handler for GetFileHandler {
impl GetFileHandler {
    fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let (parts, _) = request.into_parts();

        // fetch and compare local and distant history
        // if == => serve local
        // if local > distant
        //   update distant
        //   serve local
        // if distant > local
        //   update distant
        //   serve distant
        // if divergence
        //   :(

        /*let res = self.file_client.forward(&parts, hyper::Body::empty()).await;

        // TODO nominal case
        match res {
            Ok(response) => {
                // if 200 of 404
                //  update local or distant if necessary
                //  forward response
                // else
                //  do error management
                todo!()
            }
            Err(_) => {
                // do error management
                todo!()
            }
        }*/
        // TODO

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
