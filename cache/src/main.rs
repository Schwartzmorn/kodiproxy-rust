#[tokio::main]
async fn main() {
    kp_cache::serve_cache().await;
}
