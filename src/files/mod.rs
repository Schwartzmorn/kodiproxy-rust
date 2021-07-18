pub mod repository;

pub use self::repository::*;

fn map_error<T: std::fmt::Debug>(e: &T, msg: &str, error_code: u16) -> crate::router::RouterError {
    crate::router::HandlerError(error_code, format!("{}: {:?}", msg, e))
}

#[cfg(test)]
pub use self::repository::tests::TestRepo;
