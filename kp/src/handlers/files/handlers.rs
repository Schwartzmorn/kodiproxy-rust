use std::convert::TryInto;

/// Handler that takes care of PUT requests
pub struct DeleteFileHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<files::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of GET requests
pub struct GetFileHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<files::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of GET requests
pub struct MoveFileHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<files::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of PUT requests
pub struct PutFileHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<files::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

pub struct FileVersionsHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<files::db::FilesDB>>,
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

fn get_path_and_name_from_uri(uri: &http::Uri) -> Result<(String, String), router::RouterError> {
    let full_path = get_path_from_uri(uri)?;
    let full_path = std::path::PathBuf::from(full_path);
    let file_path = full_path
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .to_string_lossy();
    let file_name = full_path
        .file_name()
        .ok_or(router::InvalidRequest(String::from("Invalid url")))?
        .to_string_lossy();
    Ok((file_path.into(), file_name.into()))
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
        let (file_path, file_name) = get_path_and_name_from_uri(&request.uri())?;

        let mut repo = self.file_repo.lock().unwrap();

        repo.delete(
            file_path.as_ref(),
            file_name.as_ref(),
            &request
                .extensions()
                .get::<std::net::SocketAddr>()
                .unwrap_or(&SOCK_ADDRESS)
                .ip(),
        )?;

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
        let (file_path, file_name) = get_path_and_name_from_uri(&request.uri())?;

        let repo = self.file_repo.lock().unwrap();

        let data = repo.get(file_path.as_ref(), file_name.as_ref())?;

        log::info!("Sent file with size {}", &data.len());

        Ok(hyper::Response::builder()
            .status(200)
            .header(
                "content-disposition",
                format!("attachment; filename=\"{}\"", file_name),
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

        let (file_path_from, file_name_from) = get_path_and_name_from_uri(&request.uri())?;
        let (file_path_to, file_name_to) = get_path_and_name_from_uri(&destination)?;

        let mut repo = self.file_repo.lock().unwrap();

        repo.move_to(
            file_path_from.as_ref(),
            file_name_from.as_ref(),
            file_path_to.as_ref(),
            file_name_to.as_ref(),
            &request
                .extensions()
                .get::<std::net::SocketAddr>()
                .unwrap_or(&SOCK_ADDRESS)
                .ip(),
        )?;

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
        let remote_address = request
            .extensions()
            .get::<std::net::SocketAddr>()
            .unwrap()
            .ip();
        let (parts, body) = request.into_parts();
        let (file_path, file_name) = get_path_and_name_from_uri(&parts.uri)?;

        let file_content = hyper::body::to_bytes(body)
            .await
            .map(|b| b.to_vec())
            .map_err(|e| super::map_error(&e, "Invalid content", 400))?;

        let mut repo = self.file_repo.lock().unwrap();

        repo.save(
            file_path.as_ref(),
            file_name.as_ref(),
            &file_content,
            &remote_address,
        )?;

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
        let (file_path, file_name) = get_path_and_name_from_uri(&request.uri())?;

        let repo = self.file_repo.lock().unwrap();
        let log = repo.get_history(file_path.as_ref(), file_name.as_ref())?;

        Ok(hyper::Response::builder()
            .status(200)
            .body(hyper::Body::from(
                serde_json::to_string(&log.entries).unwrap(),
            ))
            .unwrap())
    }
}

lazy_static::lazy_static!(static ref SOCK_ADDRESS: std::net::SocketAddr = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)), 0););

#[cfg(test)]
mod tests {
    use router::Handler;
    use test_log::test;

    static TEST_PATH: &str = "target/test/file_handlers_tests";

    lazy_static::lazy_static!(static ref ADDRESS: std::net::IpAddr = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)););

    fn get_repo(path: &str) -> std::sync::Arc<std::sync::Mutex<files::db::FilesDB>> {
        let path = std::path::PathBuf::from(TEST_PATH).join(path);
        if path.exists() {
            std::fs::remove_dir_all(&path)
                .expect(format!("Failed to clean folder {:?}", path).as_str());
        }
        std::sync::Arc::new(std::sync::Mutex::new(
            files::db::FilesDB::new(path).unwrap(),
        ))
    }

    #[test(tokio::test)]
    async fn it_replies_with_the_last_version() {
        let file_repo = get_repo("get");
        {
            let mut repo = file_repo.lock().unwrap();

            repo.save(
                "keepass",
                "pdb.kdbx",
                "content of current file".as_bytes().to_owned().as_ref(),
                &ADDRESS,
            )
            .unwrap();
        }

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx")
            .method("GET")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::GetFileHandler {
            file_repo,
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

    #[test(tokio::test)]
    async fn it_deletes() {
        let file_repo = get_repo("delete");
        {
            let mut repo = file_repo.lock().unwrap();

            repo.save(
                "keepass",
                "pdb.kdbx",
                "content of current file".as_bytes().to_owned().as_ref(),
                &ADDRESS,
            )
            .unwrap();
        }

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx")
            .method("DELETE")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::DeleteFileHandler {
            file_repo: file_repo.clone(),
            matcher: crate::handlers::files::get_matcher(&hyper::Method::DELETE),
        };

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(204, parts.status);

        {
            let repo = file_repo.lock().unwrap();

            let result = repo.get("keepass", "pdb.kdbx").unwrap_err();

            println!("{:?}", result);

            assert!(matches!(result, router::RouterError::HandlerError(404, _)));
        }
    }

    #[test(tokio::test)]
    async fn it_moves() {
        let file_repo = get_repo("move");
        {
            let mut repo = file_repo.lock().unwrap();

            repo.save(
                "keepass",
                "pdb.kdbx.tmp",
                "content of current file".as_bytes().to_owned().as_ref(),
                &ADDRESS,
            )
            .unwrap();
        }

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx.tmp")
            .header("destination", "/files/keepass/pdb.kdbx")
            .method("MOVE")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::MoveFileHandler {
            file_repo: file_repo.clone(),
            matcher: crate::handlers::files::get_matcher("MOVE"),
        };

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(200, parts.status);

        // TODO check from and to ?

        let req = hyper::Request::builder()
            .uri("/file-versions/keepass/pdb.kdbx.tmp")
            .method("GET")
            .body(hyper::Body::empty())
            .unwrap();

        let versions_handlers = super::FileVersionsHandler {
            file_repo: file_repo.clone(),
            matcher: crate::handlers::files::get_matcher("GET"),
        };

        let (parts, body) = versions_handlers.handle(req).await.unwrap().into_parts();

        assert_eq!(200, parts.status);

        let body = hyper::body::to_bytes(body).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        let re = regex::Regex::new(r#"^\[\{"timestamp":"[^"]+","address":"127.0.0.1","entry":\{"type":"Creation","version":0,"hash":"[^"]+"}},\{"timestamp":"[^"]+","address":"0.0.0.0","entry":\{"type":"MoveTo","version":1,"pathTo":"keepass/pdb.kdbx"}}\]$"#).unwrap();
        log::error!("{}", body);
        assert!(re.is_match(&body));
    }
}
