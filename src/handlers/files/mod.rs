mod handlers;

fn map_error<T: std::fmt::Debug>(e: &T, msg: &str, error_code: u16) -> crate::router::RouterError {
    crate::router::HandlerError(error_code, format!("{}: {:?}", msg, e))
}

fn get_matcher<T>(method: T) -> Box<dyn crate::router::matcher::Matcher>
where
    hyper::Method: std::convert::TryFrom<T>,
{
    crate::router::matcher::builder()
        .regex_path("^/files/")
        .with_method(method)
        .build()
        .unwrap()
}

pub fn get_file_handlers(
    configuration: &crate::configuration::FileConfiguration,
) -> Vec<Box<dyn crate::router::Handler>> {
    let file_repo = crate::files::FileRepository::new(&configuration.root_path);
    log::info!(
        "Initializing file repository in {:?}",
        &configuration.root_path
    );
    vec![
        Box::from(handlers::DeleteFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher(&hyper::Method::DELETE),
        }),
        Box::from(handlers::GetFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher(&hyper::Method::GET),
        }),
        Box::from(handlers::MoveFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher("MOVE"),
        }),
        Box::from(handlers::PutFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher(&hyper::Method::PUT),
        }),
    ]
}
