use super::repository::FileRepository;
use std::convert::TryInto;

/// Handler that takes care of PUT requests
pub struct DeleteFileHandler {
    pub file_repo: std::sync::Arc<FileRepository>,
    pub matcher: Box<dyn crate::router::matcher::Matcher>,
}

/// Handler that takes care of GET requests
pub struct GetFileHandler {
    pub file_repo: std::sync::Arc<FileRepository>,
    pub matcher: Box<dyn crate::router::matcher::Matcher>,
}

/// Handler that takes care of GET requests
pub struct MoveFileHandler {
    pub file_repo: std::sync::Arc<FileRepository>,
    pub matcher: Box<dyn crate::router::matcher::Matcher>,
}

/// Handler that takes care of PUT requests
pub struct PutFileHandler {
    pub file_repo: std::sync::Arc<FileRepository>,
    pub matcher: Box<dyn crate::router::matcher::Matcher>,
}

fn get_path_from_uri(uri: &http::Uri) -> &str {
    &uri.path()[7..]
}

#[async_trait::async_trait]
impl crate::router::Handler for DeleteFileHandler {
    fn get_matcher(&self) -> &Box<dyn crate::router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        let repo = self
            .file_repo
            .get_single_file_repo(get_path_from_uri(&request.uri()), false)?;

        repo.delete()?;

        Ok(hyper::Response::builder()
            .status(204)
            .body(hyper::Body::empty())
            .unwrap())
    }
}

#[async_trait::async_trait]
impl crate::router::Handler for GetFileHandler {
    fn get_matcher(&self) -> &Box<dyn crate::router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        let repo = self
            .file_repo
            .get_single_file_repo(get_path_from_uri(&request.uri()), false)?;

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

#[async_trait::async_trait]
impl crate::router::Handler for MoveFileHandler {
    fn get_matcher(&self) -> &Box<dyn crate::router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        let destination: http::Uri = request
            .headers()
            .get("destination")
            .ok_or(crate::router::RouterError::HandlerError(
                400,
                String::from("Missing destination"),
            ))?
            .to_str()
            .map_err(|e| super::map_error(&e, "Invalid destination", 400))?
            .try_into()
            .map_err(|e| super::map_error(&e, "Invalid destination", 400))?;

        let destination = self.file_repo.get_full_dir(get_path_from_uri(&destination));

        let repo = self
            .file_repo
            .get_single_file_repo(get_path_from_uri(&request.uri()), false)?;

        repo.rename(&destination)?;

        Ok(hyper::Response::builder()
            .status(200)
            .body(hyper::Body::empty())
            .unwrap())
    }
}

#[async_trait::async_trait]
impl crate::router::Handler for PutFileHandler {
    fn get_matcher(&self) -> &Box<dyn crate::router::matcher::Matcher> {
        &self.matcher
    }

    async fn handle(
        &self,
        request: hyper::Request<hyper::Body>,
    ) -> Result<hyper::Response<hyper::Body>, crate::router::RouterError> {
        let (parts, body) = request.into_parts();
        let file_content = hyper::body::to_bytes(body)
            .await
            .map(|b| b.to_vec())
            .map_err(|e| super::map_error(&e, "Invalid content", 400))?;

        let repo = self
            .file_repo
            .get_single_file_repo(get_path_from_uri(&parts.uri), true)?;

        repo.save(&file_content)?;

        Ok(hyper::Response::builder()
            .status(201)
            .body(hyper::Body::empty())
            .unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::router::Handler;

    use std::io::prelude::*;

    static TEST_PATH: &str = "target/test/file_handlers_tests";

    struct TestRepo {
        test_path: std::path::PathBuf,
        repo: std::sync::Arc<super::FileRepository>,
    }

    impl TestRepo {
        fn new(test_name: &str) -> TestRepo {
            let test_path = std::path::PathBuf::from(TEST_PATH).join(test_name);
            TestRepo::clean(&test_path);
            TestRepo {
                test_path: test_path.to_owned(),
                repo: super::FileRepository::new(test_path),
            }
        }

        fn clean(path: &std::path::PathBuf) {
            if path.exists() {
                std::fs::remove_dir_all(path).unwrap();
            }
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            TestRepo::clean(&self.test_path);
        }
    }

    #[rstest::fixture]
    fn file_repo(#[default("test")] test_name: &str) -> TestRepo {
        TestRepo::new(test_name)
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn it_replies_with_the_last_version(#[with("get")] file_repo: TestRepo) {
        let path = file_repo.repo.get_full_dir("keepass/pdb.kdbx");
        std::fs::create_dir_all(&path).unwrap();
        for n in 0..5 {
            let file_path = path.join(n.to_string());
            let mut file = std::fs::File::create(&file_path).expect("Could not create file");
            file.write_all(format!("content of file {}", n).as_bytes())
                .unwrap();
        }
        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx")
            .method("GET")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::GetFileHandler {
            file_repo: file_repo.repo.clone(),
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

        assert_eq!("content of file 4", content);
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn it_deletes(#[with("delete")] file_repo: TestRepo) {
        let path = file_repo.repo.get_full_dir("keepass/pdb.kdbx");
        std::fs::create_dir_all(&path).unwrap();
        for n in 0..2 {
            let file_path = path.join(n.to_string());
            let mut file = std::fs::File::create(&file_path).expect("Could not create file");
            file.write_all(format!("content of file {}", n).as_bytes())
                .unwrap();
        }

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx")
            .method("DELETE")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::DeleteFileHandler {
            file_repo: file_repo.repo.clone(),
            matcher: crate::handlers::files::get_matcher(&hyper::Method::DELETE),
        };

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(204, parts.status);

        assert!(!path.exists());
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn it_moves(#[with("move")] file_repo: TestRepo) {
        let path_from = file_repo.repo.get_full_dir("keepass/pdb.kdbx.tmp");
        let path_to = file_repo.repo.get_full_dir("keepass/pdb.kdbx");
        std::fs::create_dir_all(&path_from).unwrap();
        for n in 0..2 {
            let file_path = path_from.join(n.to_string());
            let mut file = std::fs::File::create(&file_path).expect("Could not create file");
            file.write_all(format!("content of file {}", n).as_bytes())
                .unwrap();
        }

        let req = hyper::Request::builder()
            .uri("/files/keepass/pdb.kdbx.tmp")
            .header("destination", "/files/keepass/pdb.kdbx")
            .method("MOVE")
            .body(hyper::Body::empty())
            .unwrap();

        let file_handler = super::MoveFileHandler {
            file_repo: file_repo.repo.clone(),
            matcher: crate::handlers::files::get_matcher("MOVE"),
        };

        let (parts, _body) = file_handler.handle(req).await.unwrap().into_parts();

        assert_eq!(200, parts.status);

        assert!(!path_from.exists());
        assert!(path_to.exists());
        assert!(path_to.join("0").exists());
        assert!(path_to.join("1").exists());
    }
}
