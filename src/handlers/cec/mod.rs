mod handlers;

fn get_matcher(path: &str) -> Box<dyn crate::router::matcher::Matcher> {
    crate::router::matcher::builder()
        .exact_path(String::from("/cec/") + path)
        .with_method(&hyper::Method::GET)
        .build()
        .unwrap()
}

pub fn get_cec_handlers(
    cec_interface: std::sync::Arc<std::sync::Mutex<dyn crate::cec::CECInterface>>,
) -> Vec<Box<dyn crate::router::Handler>> {
    vec![
        Box::from(handlers::CECPowerOn {
            connection: cec_interface.clone(),
            matcher: get_matcher("power-on"),
        }),
        Box::from(handlers::CECStandby {
            connection: cec_interface.clone(),
            matcher: get_matcher("standby"),
        }),
    ]
}
