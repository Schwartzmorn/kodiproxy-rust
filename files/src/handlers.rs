use std::convert::TryInto;

/// Handler that takes care of DELETE requests
pub struct DeleteFileHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<crate::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of GET requests
pub struct GetFileHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<crate::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of MOVE requests
pub struct MoveFileHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<crate::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

/// Handler that takes care of PUT requests
pub struct PutFileHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<crate::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

pub struct FileVersionsHandler {
    pub file_repo: std::sync::Arc<std::sync::Mutex<crate::db::FilesDB>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

fn get_response_builder(data: &crate::db::FilesDbResponse, status: u16) -> http::response::Builder {
    hyper::Response::builder()
        .status(status)
        .header("last-modified", data.timestamp.to_rfc2822())
        .header("etag", format!("\"{}\"", data.version))
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
        let (file_path, file_name) = crate::get_path_and_name_from_uri(&request.uri())?;
        let (version, _timestamp) = super::get_version_info_from_headers(&request.headers());
        let version = version.ok_or(router::HandlerError(400, String::from("Missing version")))?;

        let mut repo = self.file_repo.lock().unwrap();

        let data = repo.delete(
            file_path.as_ref(),
            file_name.as_ref(),
            version,
            &request
                .extensions()
                .get::<std::net::SocketAddr>()
                .unwrap_or(&DEFAULT_SOCK_ADDRESS)
                .ip(),
        )?;

        Ok(get_response_builder(&data, 204)
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
        let (file_path, file_name) = crate::get_path_and_name_from_uri(&request.uri())?;

        let is_get = request.method() == http::Method::GET;

        let repo = self.file_repo.lock().unwrap();

        let data = repo.get(file_path.as_ref(), file_name.as_ref(), is_get)?;

        log::info!(
            "Sending file with size {}",
            &data.file.as_ref().unwrap().len()
        );

        Ok(get_response_builder(&data, 200)
            .header(
                "content-disposition",
                format!("attachment; filename=\"{}\"", file_name),
            )
            .body(if is_get {
                hyper::Body::from(data.file.unwrap())
            } else {
                hyper::Body::empty()
            })
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

        let (file_path_from, file_name_from) = crate::get_path_and_name_from_uri(&request.uri())?;
        let (file_path_to, file_name_to) = crate::get_path_and_name_from_uri(&destination)?;
        let (version, _timestamp) = super::get_version_info_from_headers(&request.headers());
        let version = version.ok_or(router::HandlerError(400, String::from("Missing version")))?;

        let mut repo = self.file_repo.lock().unwrap();

        let data = repo.move_to(
            file_path_from.as_ref(),
            file_name_from.as_ref(),
            version,
            file_path_to.as_ref(),
            file_name_to.as_ref(),
            &request
                .extensions()
                .get::<std::net::SocketAddr>()
                .unwrap_or(&DEFAULT_SOCK_ADDRESS)
                .ip(),
        )?;

        Ok(get_response_builder(&data, 204)
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
        let (file_path, file_name) = crate::get_path_and_name_from_uri(&parts.uri)?;
        let (version, _timestamp) = super::get_version_info_from_headers(&parts.headers);

        let file_content = hyper::body::to_bytes(body)
            .await
            .map(|b| b.to_vec())
            .map_err(|e| super::map_error(&e, "Invalid content", 400))?;

        let mut repo = self.file_repo.lock().unwrap();

        let data = repo.save(
            file_path.as_ref(),
            file_name.as_ref(),
            &file_content,
            version,
            &remote_address,
        )?;

        Ok(get_response_builder(&data, 201)
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
        let (file_path, file_name) = crate::get_path_and_name_from_uri(&request.uri())?;

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

lazy_static::lazy_static!(
    static ref DEFAULT_SOCK_ADDRESS: std::net::SocketAddr
        = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)), 0);
);

#[cfg(test)]
mod tests {
    use router::Handler;
    use test_log::test;

    static TEST_PATH: &str = "target/test/file_handlers_tests";

    lazy_static::lazy_static!(static ref ADDRESS: std::net::IpAddr = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)););

    fn get_repo(path: &str) -> std::sync::Arc<std::sync::Mutex<crate::db::FilesDB>> {
        let path = std::path::PathBuf::from(TEST_PATH).join(path);
        if path.exists() {
            std::fs::remove_dir_all(&path)
                .expect(format!("Failed to clean folder {:?}", path).as_str());
        }
        std::sync::Arc::new(std::sync::Mutex::new(
            crate::db::FilesDB::new(path).unwrap(),
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
                None,
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
            matcher: crate::get_matcher(&hyper::Method::GET),
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
        assert_eq!(
            "\"0\"",
            parts.headers.get("ETag").unwrap().to_str().unwrap()
        );
        assert!(parts.headers.contains_key("Last-Modified"));

        let content =
            String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap();

        assert_eq!("content of current file", content);

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx")
            .method("HEAD")
            .header("ETag", "\"0\"")
            .body(hyper::Body::empty())
            .unwrap();

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();
        assert_eq!(200, parts.status);
        assert_eq!(
            "attachment; filename=\"pdb.kdbx\"",
            parts
                .headers
                .get("Content-Disposition")
                .unwrap()
                .to_str()
                .unwrap()
        );
        assert_eq!(
            "\"0\"",
            parts.headers.get("ETag").unwrap().to_str().unwrap()
        );
        assert!(parts.headers.contains_key("Last-Modified"));
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
                None,
                &ADDRESS,
            )
            .unwrap();
        }

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx")
            .method("DELETE")
            .header("ETag", "\"0\"")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::DeleteFileHandler {
            file_repo: file_repo.clone(),
            matcher: crate::get_matcher(&hyper::Method::DELETE),
        };

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(204, parts.status);
        assert_eq!(
            "\"1\"",
            parts.headers.get("ETag").unwrap().to_str().unwrap()
        );

        {
            let repo = file_repo.lock().unwrap();

            let result = repo.get("keepass", "pdb.kdbx", true).unwrap_err();

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
                None,
                &ADDRESS,
            )
            .unwrap();
        }

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx.tmp")
            .header("destination", "/files/keepass/pdb.kdbx")
            .method("MOVE")
            .header("ETag", "\"0\"")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::MoveFileHandler {
            file_repo: file_repo.clone(),
            matcher: crate::get_matcher("MOVE"),
        };

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(204, parts.status);

        // TODO check from and to ?

        let req = hyper::Request::builder()
            .uri("/file-versions/keepass/pdb.kdbx.tmp")
            .method("GET")
            .body(hyper::Body::empty())
            .unwrap();

        let versions_handlers = super::FileVersionsHandler {
            file_repo: file_repo.clone(),
            matcher: crate::get_matcher("GET"),
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
