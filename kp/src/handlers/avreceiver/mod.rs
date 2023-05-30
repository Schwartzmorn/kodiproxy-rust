mod handlers;

fn get_matcher(path: &str) -> Box<dyn router::matcher::Matcher> {
    router::matcher::builder()
        .exact_path(String::from("/avreceiver/") + path)
        .with_method(&hyper::Method::GET)
        .build()
        .unwrap()
}

pub fn get_handlers(
    receiver: std::sync::Arc<dyn crate::avreceiver::AVReceiverInterface>,
) -> Vec<Box<dyn router::Handler>> {
    vec![
        Box::from(handlers::AVReceiverVolumeHandler {
            receiver: receiver.clone(),
            matcher: get_matcher("volume"),
        }),
        Box::from(handlers::AVReceiverPowerHandler {
            receiver: receiver.clone(),
            matcher: get_matcher("power"),
        }),
    ]
}
