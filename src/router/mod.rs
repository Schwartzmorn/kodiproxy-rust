pub use self::router::{Handler, Router, RouterError};

pub mod matcher;
pub mod router;

pub fn parse_url(url: &String) -> (String, String, Option<String>) {
    let url_re: regex::Regex =
        regex::Regex::new(r"^(?P<scheme>https?)://(?P<authority>[^/]+)(?P<path>.*)").unwrap();

    let captures = url_re
        .captures(url.as_str())
        .expect("Incorrect url for the jsonrpc server");

    (
        String::from(&captures["scheme"]),
        String::from(&captures["authority"]),
        if captures["path"].len() > 0 {
            Some(String::from(&captures["path"]))
        } else {
            None
        },
    )
}
