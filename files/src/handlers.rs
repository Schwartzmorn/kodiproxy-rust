use std::convert::TryInto;

type FileRepo = std::sync::Arc<std::sync::Mutex<crate::db::FilesDB>>;

pub fn add_routes(router: axum::Router, file_repo: FileRepo) -> axum::Router {
    let files_router = axum::Router::new()
        .route(
            "/{*path}",
            axum::routing::any(move_file)
                .get(get_file)
                .head(head_file)
                .put(put_file)
                .delete(delete_file),
        )
        .route_layer(tower_http::timeout::TimeoutLayer::with_status_code(
            http::StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(1),
        ))
        .with_state(file_repo.clone());
    let versions_router = axum::Router::new()
        .route("/{*path}", axum::routing::get(file_versions))
        .route_layer(tower_http::timeout::TimeoutLayer::with_status_code(
            http::StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(1),
        ))
        .with_state(file_repo);

    router
        .nest("/files", files_router)
        .nest("/file-versions", versions_router)
}

fn error_response(error: router::RouterError) -> hyper::Response<axum::body::Body> {
    let (status, message) = match error {
        router::RouterError::ForwardingError(message) => (502, message),
        router::RouterError::HandlerError(status, message) => (status, message),
        router::RouterError::InvalidRequest(message) => (400, message),
        router::RouterError::MethodNotAllowed => (405, String::from("Method Not Allowed")),
        router::RouterError::NotFound => (404, String::from("Not Found")),
    };

    hyper::Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(axum::body::Body::from(message))
        .unwrap()
}

#[derive(Debug)]
struct FileError(router::RouterError);

impl From<router::RouterError> for FileError {
    fn from(error: router::RouterError) -> Self {
        Self(error)
    }
}

impl axum::response::IntoResponse for FileError {
    fn into_response(self) -> axum::response::Response {
        axum::response::IntoResponse::into_response(error_response(self.0))
    }
}

type FileResult = Result<hyper::Response<axum::body::Body>, FileError>;

fn get_response_builder(data: &crate::db::FilesDbResponse, status: u16) -> http::response::Builder {
    hyper::Response::builder()
        .status(status)
        .header("last-modified", data.timestamp.to_rfc2822())
        .header("etag", format!("\"{}\"", data.version))
}

async fn delete_file(
    axum::extract::State(file_repo): axum::extract::State<FileRepo>,
    axum::extract::Path(path): axum::extract::Path<String>,
    axum::extract::ConnectInfo(remote_address): axum::extract::ConnectInfo<std::net::SocketAddr>,
    headers: http::HeaderMap,
) -> FileResult {
    let (file_path, file_name) = crate::get_path_and_name_from_path(&path)?;
    let (version, _timestamp) = super::get_version_info_from_headers(&headers);
    let version = version.ok_or(router::HandlerError(400, String::from("Missing version")))?;

    let mut repo = file_repo.lock().unwrap();

    let data = repo.delete(
        file_path.as_ref(),
        file_name.as_ref(),
        version,
        &remote_address.ip(),
    )?;

    Ok(get_response_builder(&data, 204)
        .body(axum::body::Body::empty())
        .unwrap())
}

async fn get_file(
    axum::extract::State(file_repo): axum::extract::State<FileRepo>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> FileResult {
    file_response(&file_repo, &path, true)
}

async fn head_file(
    axum::extract::State(file_repo): axum::extract::State<FileRepo>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> FileResult {
    file_response(&file_repo, &path, false)
}

fn file_response(file_repo: &FileRepo, path: &str, include_content: bool) -> FileResult {
    let (file_path, file_name) = crate::get_path_and_name_from_path(path)?;

    let repo = file_repo.lock().unwrap();

    let data = repo.get(file_path.as_ref(), file_name.as_ref(), include_content)?;

    if include_content {
        log::info!(
            "Sending file with size {}",
            data.file.as_ref().unwrap().len()
        );
    }

    Ok(get_response_builder(&data, 200)
        .header(
            "content-disposition",
            format!("attachment; filename=\"{}\"", file_name),
        )
        .body(if include_content {
            axum::body::Body::from(data.file.unwrap())
        } else {
            axum::body::Body::empty()
        })
        .unwrap())
}

async fn move_file(
    axum::extract::State(file_repo): axum::extract::State<FileRepo>,
    axum::extract::Path(path): axum::extract::Path<String>,
    axum::extract::ConnectInfo(remote_address): axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: hyper::Request<axum::body::Body>,
) -> FileResult {
    if request.method().as_str() != "MOVE" {
        return Err(router::RouterError::MethodNotAllowed.into());
    }

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

    let (file_path_from, file_name_from) = crate::get_path_and_name_from_path(&path)?;
    let (file_path_to, file_name_to) = crate::get_path_and_name_from_uri(&destination)?;
    let (version, _timestamp) = super::get_version_info_from_headers(request.headers());
    let version = version.ok_or(router::HandlerError(400, String::from("Missing version")))?;

    let mut repo = file_repo.lock().unwrap();

    let data = repo.move_to(
        file_path_from.as_ref(),
        file_name_from.as_ref(),
        version,
        file_path_to.as_ref(),
        file_name_to.as_ref(),
        &remote_address.ip(),
    )?;

    Ok(get_response_builder(&data, 204)
        .body(axum::body::Body::empty())
        .unwrap())
}

async fn put_file(
    axum::extract::State(file_repo): axum::extract::State<FileRepo>,
    axum::extract::Path(path): axum::extract::Path<String>,
    axum::extract::ConnectInfo(remote_address): axum::extract::ConnectInfo<std::net::SocketAddr>,
    headers: http::HeaderMap,
    request: hyper::Request<axum::body::Body>,
) -> FileResult {
    let (file_path, file_name) = crate::get_path_and_name_from_path(&path)?;
    let (version, _timestamp) = super::get_version_info_from_headers(&headers);

    let file_content = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .map(|b| b.to_vec())
        .map_err(|e| super::map_error(&e, "Invalid content", 400))?;

    let mut repo = file_repo.lock().unwrap();

    let data = repo.save(
        file_path.as_ref(),
        file_name.as_ref(),
        &file_content,
        version,
        &remote_address.ip(),
    )?;

    Ok(get_response_builder(&data, 201)
        .body(axum::body::Body::empty())
        .unwrap())
}

async fn file_versions(
    axum::extract::State(file_repo): axum::extract::State<FileRepo>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> FileResult {
    let (file_path, file_name) = crate::get_path_and_name_from_path(&path)?;

    let repo = file_repo.lock().unwrap();
    let log = repo.get_history(file_path.as_ref(), file_name.as_ref())?;

    Ok(hyper::Response::builder()
        .status(200)
        .body(axum::body::Body::from(
            serde_json::to_string(&log.entries).unwrap(),
        ))
        .unwrap())
}

#[cfg(test)]
mod tests {
    use test_log::test;

    static TEST_PATH: &str = "target/test/file_handlers_tests";

    lazy_static::lazy_static!(static ref ADDRESS: std::net::IpAddr = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)););

    fn get_repo(path: &str) -> std::sync::Arc<std::sync::Mutex<crate::db::FilesDB>> {
        let path = std::path::PathBuf::from(TEST_PATH).join(path);
        if path.exists() {
            std::fs::remove_dir_all(&path)
                .unwrap_or_else(|_| panic!("Failed to clean folder {:?}", path));
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

        let (parts, body) = super::get_file(
            axum::extract::State(file_repo.clone()),
            axum::extract::Path(String::from("keepass/pdb.kdbx")),
        )
        .await
        .unwrap()
        .into_parts();

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

        let content = String::from_utf8(
            axum::body::to_bytes(body, usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();

        assert_eq!("content of current file", content);

        let (parts, _body) = super::head_file(
            axum::extract::State(file_repo),
            axum::extract::Path(String::from("keepass/pdb.kdbx")),
        )
        .await
        .unwrap()
        .into_parts();
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

        let mut headers = http::HeaderMap::new();
        headers.insert("etag", http::HeaderValue::from_static("\"0\""));

        let (parts, _body) = super::delete_file(
            axum::extract::State(file_repo.clone()),
            axum::extract::Path(String::from("keepass/pdb.kdbx")),
            axum::extract::ConnectInfo(std::net::SocketAddr::new(*ADDRESS, 0)),
            headers,
        )
        .await
        .unwrap()
        .into_parts();

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
            .body(axum::body::Body::empty())
            .unwrap();

        let (parts, _body) = super::move_file(
            axum::extract::State(file_repo.clone()),
            axum::extract::Path(String::from("keepass/pdb.kdbx.tmp")),
            axum::extract::ConnectInfo(std::net::SocketAddr::new(*ADDRESS, 0)),
            req,
        )
        .await
        .unwrap()
        .into_parts();

        assert_eq!(204, parts.status);

        // TODO check from and to ?

        let (parts, body) = super::file_versions(
            axum::extract::State(file_repo.clone()),
            axum::extract::Path(String::from("keepass/pdb.kdbx.tmp")),
        )
        .await
        .unwrap()
        .into_parts();

        assert_eq!(200, parts.status);

        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        let re = regex::Regex::new(r#"^\[\{"timestamp":"[^"]+","address":"127\.0\.0\.1","entry":\{"type":"Creation","version":0,"hash":"[^"]+"}},\{"timestamp":"[^"]+","address":"127\.0\.0\.1","entry":\{"type":"MoveTo","version":1,"pathTo":"keepass/pdb\.kdbx"}}\]$"#).unwrap();
        log::error!("{}", body);
        assert!(re.is_match(&body));
    }
}
