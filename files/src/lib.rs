pub mod log;
pub mod repository;
pub mod tests;

pub use self::repository::*;

fn map_error<T: std::fmt::Debug>(e: &T, msg: &str, error_code: u16) -> router::RouterError {
    router::HandlerError(error_code, format!("{}: {:?}", msg, e))
}
