pub mod db;
pub mod log;

fn map_error<E: std::fmt::Debug>(e: &E, msg: &str, error_code: u16) -> router::RouterError {
    ::log::info!("Got error: {:?}", e);
    router::HandlerError(error_code, format!("{}: {:?}", msg, e))
}
