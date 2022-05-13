pub mod client;
pub mod db;
pub mod handlers;
use std::str::FromStr;

pub struct SyncInformation {
    pub last_synced_version: i32,
    pub last_synced_timestamp: chrono::DateTime<chrono::Utc>,
}

fn map_error<E: std::fmt::Debug, S: std::fmt::Display>(
    e: &E,
    msg: S,
    error_code: u16,
) -> router::RouterError {
    ::log::warn!("{}: {:?}", msg, e);
    router::HandlerError(error_code, format!("{}: {:?}", msg, e))
}

fn register_handlers(_router: &mut router::Router) {
    todo!();
}

pub async fn serve_cache() {
    let addr = std::net::SocketAddr::from_str("[::]:3000")
        .expect("Incorrect host in server configuration");

    router::serve(addr, None, |router| register_handlers(router)).await;
}
