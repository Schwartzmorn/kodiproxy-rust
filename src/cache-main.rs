pub mod configuration;
pub mod files;
pub mod router;

fn register_handlers(router: &mut router::Router) {
    let conf = configuration::FileConfiguration {
        root_path: std::path::PathBuf::from("target/test/cache"),
    };

    router.add_handlers(files::get_file_handlers(&conf));
}

fn setup_logging() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Trace)
        .target(env_logger::Target::Stdout)
        .init();
}

#[tokio::main]
async fn main() {
    setup_logging();
    let conf = configuration::ServerConfiguration {
        host: String::from("[::]:3000"),
    };

    router::serve(&conf, None, register_handlers).await;
}
