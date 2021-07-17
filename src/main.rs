fn parse_args() -> clap::ArgMatches<'static> {
    clap::App::new("KodiProxy")
        .author("Schwartzmorn")
        .about("My one stop proxy for my pi")
        .arg(
            clap::Arg::with_name("configuration")
                .short("c")
                .long("configuration")
                .value_name("FILE")
                .help("path to the configuration file of the different modules"),
        )
        .arg(
            clap::Arg::with_name("dump_configuration")
                .long("dump_configuration")
                .value_name("OUT_FILE")
                .help("dump the configuration to file and exit"),
        )
        .get_matches()
}

fn get_configuration(path: Option<&str>) -> kp::configuration::ProxyConfiguration {
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
    let args = parse_args();

    let configuration = get_configuration(args.value_of("configuration"));

    setup_logging(&configuration.logging);

    if let Some(path) = args.value_of("dump_configuration") {
        dump_configuration(path, configuration);
        return;
    }

    kp::serve(&configuration, None).await;
}
