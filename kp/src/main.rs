use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(name = "KodiProxy")]
#[command(author = "Schwartzmorn")]
#[command(about = "My one stop proxy for my pi", long_about = None)]
struct Args {
    #[arg(long)]
    #[arg(help = "dump the configuration to file and exit")]
    #[arg(value_name = "OUT_FILE")]
    dump_configuration: Option<String>,

    #[arg(short)]
    #[arg(long)]
    #[arg(help = "path to the configuration file of the different modules")]
    #[arg(value_name = "FILE")]
    configuration: Option<String>,
}

fn get_configuration(path: &Option<String>) -> kp::configuration::ProxyConfiguration {
    match path {
        Some(path) => {
            let configuration = std::fs::read(path).expect("Configuration file not found");
            let configuration =
                String::from_utf8(configuration).expect("Could not decode the configuration file");
            serde_json::from_str(configuration.as_str()).expect("Invalid configuration file")
        }
        None => serde_json::from_str("{}").unwrap(),
    }
}

fn dump_configuration(path: &str, configuration: kp::configuration::ProxyConfiguration) {
    println!("Dumping configuration to {}", path);
    std::fs::write(
        path,
        serde_json::to_string_pretty(&configuration).expect("Failed to serialize configuration"),
    )
    .expect("Failed to write configuration");
}

fn setup_logging(configuration: &kp::configuration::LoggingConfiguration) {
    // TODO take target into account
    let level = if configuration.enabled {
        configuration.level
    } else {
        log::LevelFilter::Off
    };
    env_logger::Builder::from_default_env()
        .filter_level(level)
        .target(env_logger::Target::Stdout)
        .init();
    log::info!("Logger initialized with level {:?}", level);
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let configuration = get_configuration(&args.configuration);

    setup_logging(&configuration.logging);

    if let &Some(path) = &args.dump_configuration.as_deref() {
        dump_configuration(path, configuration);
        return;
    }

    kp::serve_kp(&configuration, None).await;
}
