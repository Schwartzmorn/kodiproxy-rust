mod file_client;
mod handlers;
pub mod logs_comparator;

use std::str::FromStr;

fn register_handlers(_router: &mut router::Router) {
    todo!();
}

pub async fn serve_cache() {
    let addr = std::net::SocketAddr::from_str("[::]:3000")
        .expect("Incorrect host in server configuration");

    router::serve(addr, None, |router| register_handlers(router)).await;
}
