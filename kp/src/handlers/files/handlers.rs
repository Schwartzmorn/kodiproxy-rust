use std::convert::TryInto;

/// Handler that takes care of PUT requests
pub struct DeleteFileHandler {
    pub file_repo: std::sync::Arc<files::FileRepository>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of GET requests
pub struct GetFileHandler {
    pub file_repo: std::sync::Arc<files::FileRepository>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of GET requests
pub struct MoveFileHandler {
    pub file_repo: std::sync::Arc<files::FileRepository>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of PUT requests
pub struct PutFileHandler {
    pub file_repo: std::sync::Arc<files::FileRepository>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

pub struct FileVersionsHandler {
    pub file_repo: std::sync::Arc<files::FileRepository>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

fn get_path_from_uri(uri: &http::Uri) -> Result<&str, router::RouterError> {
    lazy_static::lazy_static! {
        static ref URI_REGEX: regex::Regex = regex::Regex::new(r"^/(files|file-versions)/(.+)").unwrap();
    }
    let matches = URI_REGEX.captures(uri.path());
    match matches {
        Some(matches) => Ok(matches.get(2).unwrap().as_str()),
        None => Err(router::InvalidRequest(String::from("Invalid url"))),
    }
}

#[async_trait::async_trait]
impl router::Handler for DeleteFileHandler {
    fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let file_path = get_path_from_uri(&request.uri())?;
        let repo = self.file_repo.get_single_file_repo(file_path, false)?;

        repo.delete()?;

        Ok(hyper::Response::builder()
            .status(204)
            .body(hyper::Body::empty())
            .unwrap())
    }
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
        let file_path = get_path_from_uri(&request.uri())?;
        let repo = self.file_repo.get_single_file_repo(file_path, false)?;

        let filename = repo.get_filename()?;

        let data = repo.get_current_version()?;

        log::info!("Sent file with size {}", &data.len());

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

#[async_trait::async_trait]
impl router::Handler for MoveFileHandler {
    fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let destination: http::Uri = request
            .headers()
            .get("destination")
            .ok_or(router::RouterError::HandlerError(
                400,
                String::from("Missing destination"),
            ))?
            .to_str()
            .map_err(|e| super::map_error(&e, "Invalid destination", 400))?
            .try_into()
            .map_err(|e| super::map_error(&e, "Invalid destination", 400))?;

        let path_from = get_path_from_uri(&request.uri())?;
        let path_to = get_path_from_uri(&destination)?;

        let destination = self.file_repo.get_single_file_repo(path_to, true)?;

        let repo = self.file_repo.get_single_file_repo(path_from, false)?;

        repo.rename(&destination)?;

        Ok(hyper::Response::builder()
            .status(200)
            .body(hyper::Body::empty())
            .unwrap())
    }
}

#[async_trait::async_trait]
impl router::Handler for PutFileHandler {
    fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let (parts, body) = request.into_parts();
        let file_path = get_path_from_uri(&parts.uri)?;

        let file_content = hyper::body::to_bytes(body)
            .await
            .map(|b| b.to_vec())
            .map_err(|e| super::map_error(&e, "Invalid content", 400))?;

        let repo = self.file_repo.get_single_file_repo(file_path, true)?;

        repo.save(&file_content)?;

        Ok(hyper::Response::builder()
            .status(201)
            .body(hyper::Body::empty())
            .unwrap())
    }
}

#[async_trait::async_trait]
impl router::Handler for FileVersionsHandler {
    fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
        let file_path = get_path_from_uri(&request.uri())?;
        let repo = self.file_repo.get_single_file_repo(file_path, false)?;

        let log = &repo.get_log().map_err(|_| router::NotFound)?;

        Ok(hyper::Response::builder()
            .status(200)
            .body(hyper::Body::from(
                serde_json::to_string(&log.entries).unwrap(),
            ))
            .unwrap())
    }
}

#[cfg(test)]
mod tests {
    use router::Handler;

    use std::io::prelude::*;

    static TEST_PATH: &str = "target/test/file_handlers_tests";

    #[rstest::fixture]
    fn file_repo(#[default("test")] test_name: &str) -> files::tests::TestRepo {
        files::tests::TestRepo::new(std::path::PathBuf::from(TEST_PATH).join(test_name))
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn it_replies_with_the_last_version(#[with("get")] file_repo: files::tests::TestRepo) {
        let path = file_repo.get_path("keepass/pdb.kdbx");
        std::fs::create_dir_all(&path).unwrap();
        let mut file = std::fs::File::create(&path.join("current")).expect("Could not create file");
        file.write_all("content of current file".as_bytes())
            .unwrap();

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx")
            .method("GET")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::GetFileHandler {
            file_repo: file_repo.get_repo(),
            matcher: crate::handlers::files::get_matcher(&hyper::Method::GET),
        };

        let (parts, body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(200, parts.status);
        assert!(parts.headers.contains_key("Content-Disposition"));
        assert_eq!(
            "attachment; filename=\"pdb.kdbx\"",
            parts
                .headers
                .get("Content-Disposition")
                .unwrap()
                .to_str()
                .unwrap()
        );

        let content =
            String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap();

        assert_eq!("content of current file", content);
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn it_deletes(#[with("delete")] file_repo: files::tests::TestRepo) {
        let path = file_repo.get_path("keepass/pdb.kdbx");
        std::fs::create_dir_all(&path).unwrap();
        let mut file = std::fs::File::create(&path.join("current")).expect("Could not create file");
        file.write_all("content of current file".as_bytes())
            .unwrap();

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx")
            .method("DELETE")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::DeleteFileHandler {
            file_repo: file_repo.get_repo(),
            matcher: crate::handlers::files::get_matcher(&hyper::Method::DELETE),
        };

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(204, parts.status);

        assert!(path.join("0").exists());
        assert!(!path.join("current").exists());
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn it_moves(#[with("move")] file_repo: files::tests::TestRepo) {
        let path_from = file_repo.get_path("keepass/pdb.kdbx.tmp");
        let path_to = file_repo.get_path("keepass/pdb.kdbx");

        std::fs::create_dir_all(&path_from).unwrap();
        let mut file =
            std::fs::File::create(&path_from.join("current")).expect("Could not create file");
        file.write_all("content of current file".as_bytes())
            .unwrap();

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx.tmp")
            .header("destination", "/files/keepass/pdb.kdbx")
            .method("MOVE")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::MoveFileHandler {
            file_repo: file_repo.get_repo(),
            matcher: crate::handlers::files::get_matcher("MOVE"),
        };

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(200, parts.status);

        assert!(path_from.join("0").exists());
        assert!(!path_from.join("current").exists());
        assert!(path_to.join("current").exists());

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx.tmp")
            .header("destination", "/files/keepass/pdb.kdbx")
            .method("MOVE")
            .body(hyper::Body::empty())
            .unwrap();

        let versions_handlers = super::FileVersionsHandler {
            file_repo: file_repo.get_repo(),
            matcher: crate::handlers::files::get_matcher("GET"),
        };

        let (parts, body) = versions_handlers.handle(req).await.unwrap().into_parts();

        assert_eq!(200, parts.status);

        let body = hyper::body::to_bytes(body).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        let re = regex::Regex::new(r#"^\[\{"timestamp":"[^"]+","address":"127.0.0.1","entry":\{"type":"MoveTo","pathTo":"keepass/pdb.kdbx"}}\]$"#).unwrap();
        assert!(re.is_match(&body));
    }
}
