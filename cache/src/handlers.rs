pub struct DeleteCacheHandler {
    pub file_client: std::sync::Arc<crate::client::FileClient>,
    pub file_repo: std::sync::Arc<std::sync::Mutex<crate::db::CacheDb>>,
    pub matcher: Box<dyn router::matcher::Matcher>,
}

// #[async_trait::async_trait]
// impl router::Handler for DeleteCacheHandler {
//     fn get_matcher(&self) -> &Box<dyn router::matcher::Matcher> {
//         &self.matcher
//     }

//     async fn handle(
//         &self,
//         request: hyper::Request<hyper::Body>,
//     ) -> Result<hyper::Response<hyper::Body>, router::RouterError> {
//         let (file_path, file_name) = files::get_path_and_name_from_uri(&request.uri())?;

//         let mut repo = self.file_repo.lock().unwrap();

//         let data = repo.delete(
//             file_path.as_ref(),
//             file_name.as_ref(),
//             &request
//                 .extensions()
//                 .get::<std::net::SocketAddr>()
//                 .unwrap_or(&DEFAULT_SOCK_ADDRESS)
//                 .ip(),
//         )?;

//         Ok(get_response_builder(&data, 204)
//             .body(hyper::Body::empty())
//             .unwrap())
//     }
// }

#[cfg(test)]
mod tests {}
