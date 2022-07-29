pub mod db;
pub mod handlers;
pub mod log;

fn map_error<E: std::fmt::Debug>(e: &E, msg: &str, error_code: u16) -> router::RouterError {
    ::log::info!("Got error: {:?}", e);
    router::HandlerError(error_code, format!("{}: {:?}", msg, e))
}

fn get_matcher<T>(method: T) -> Box<dyn router::matcher::Matcher>
where
    hyper::Method: std::convert::TryFrom<T>,
{
    router::matcher::builder()
        .regex_path("^/files/")
        .with_method(method)
        .build()
        .unwrap()
}

pub fn get_version_info_from_headers(
    headers: &http::HeaderMap,
) -> (Option<i32>, chrono::DateTime<chrono::Utc>) {
    lazy_static::lazy_static! {
        static ref ETAG_REGEX: regex::Regex = regex::Regex::new(r#"\s*"(\d+)"\s*"#).unwrap();
    }

    let etag = headers
        .get("etag")
        .and_then(|etag| etag.to_str().ok())
        .and_then(|h| {
            ETAG_REGEX
                .captures(h)
                .and_then(|m| m.get(1).map(|c| c.as_str()))
        })
        .and_then(|etag| etag.parse().ok());

    let timestamp = headers
        .get("last-modified")
        .and_then(|timestamp| timestamp.to_str().ok())
        .and_then(|timestamp| {
            chrono::DateTime::parse_from_rfc3339(timestamp.as_ref())
                .map(|ts| ts.with_timezone(&chrono::Utc))
                .ok()
        })
        .unwrap_or(chrono::Utc::now());

    (etag, timestamp)
}

fn get_path_from_uri(uri: &http::Uri) -> Result<&str, router::RouterError> {
    lazy_static::lazy_static! {
        static ref URI_REGEX: regex::Regex = regex::Regex::new(r"^/(files|file-versions)/(.+)").unwrap();
    }
    let matches = URI_REGEX.captures(uri.path());
    match matches {
        Some(matches) => Ok(matches.get(2).unwrap().as_str()),
        None => Err(router::InvalidRequest(String::from("Invalid url"))),
    }
}

pub fn get_path_and_name_from_uri(
    uri: &http::Uri,
) -> Result<(String, String), router::RouterError> {
    let full_path = get_path_from_uri(uri)?;
    let full_path = std::path::PathBuf::from(full_path);
    let file_path = full_path
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .to_string_lossy();
    let file_name = full_path
        .file_name()
        .ok_or(router::InvalidRequest(String::from("Invalid url")))?
        .to_string_lossy();
    Ok((file_path.into(), file_name.into()))
}

pub fn get_file_handlers(sqlite_path: &std::path::PathBuf) -> Vec<Box<dyn router::Handler>> {
    let file_repo = std::sync::Arc::new(std::sync::Mutex::new(
        crate::db::FilesDB::new(&sqlite_path).unwrap(),
    ));
    ::log::info!("Initializing file repository in {:?}", &sqlite_path);
    vec![
        Box::from(handlers::DeleteFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher(&hyper::Method::DELETE),
        }),
        Box::from(handlers::GetFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher(&hyper::Method::GET),
        }),
        Box::from(handlers::GetFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher(&hyper::Method::HEAD),
        }),
        Box::from(handlers::MoveFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher("MOVE"),
        }),
        Box::from(handlers::PutFileHandler {
            file_repo: file_repo.clone(),
            matcher: get_matcher(&hyper::Method::PUT),
        }),
        Box::from(handlers::FileVersionsHandler {
            file_repo: file_repo.clone(),
            matcher: router::matcher::builder()
                .regex_path("^/file-versions/")
                .with_method(&hyper::Method::GET)
                .build()
                .unwrap(),
        }),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn get_path_and_name_from_uri() {
        let uri = http::Uri::from_static("http://fakedomain/files/test/truc.txt");
        let (path, file) = super::get_path_and_name_from_uri(&uri).expect("Failed to decode");

        assert_eq!(String::from("test"), path, "Wrong path");
        assert_eq!(String::from("truc.txt"), file, "Wrong path");

        let uri = http::Uri::from_static("http://fakedomain/files/testme");
        let (path, file) = super::get_path_and_name_from_uri(&uri).expect("Failed to decode");

        assert_eq!(String::from(""), path, "Wrong path");
        assert_eq!(String::from("testme"), file, "Wrong path");
    }

    #[test]
    fn get_version_info_from_headers() {
        let mut headers = http::HeaderMap::new();

        headers.append("etag", http::HeaderValue::from_static("\"18\""));
        headers.append(
            "last-modified",
            http::HeaderValue::from_static("2022-09-24T06:00:00Z"),
        );

        let (version, datetime) = super::get_version_info_from_headers(&headers);

        let expected_datetime = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::new(
                chrono::NaiveDate::from_ymd(2022, 09, 24),
                chrono::NaiveTime::from_hms(6, 0, 0),
            ),
            chrono::Utc,
        );

        assert_eq!(Some(18), version, "Wrong version decoded");
        assert_eq!(expected_datetime, datetime, "Wrong datetime decoded");
    }

    #[test]
    fn get_version_info_from_headers_no_version() {
        let mut headers = http::HeaderMap::new();

        headers.append(
            "last-modified",
            http::HeaderValue::from_static("2022-12-31T18:00:00.520Z"),
        );

        let (version, datetime) = super::get_version_info_from_headers(&headers);

        let expected_datetime = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::new(
                chrono::NaiveDate::from_ymd(2022, 12, 31),
                chrono::NaiveTime::from_hms_milli(18, 0, 0, 520),
            ),
            chrono::Utc,
        );

        assert_eq!(None, version, "Wrong version decoded");
        assert_eq!(expected_datetime, datetime, "Wrong datetime decoded");
    }

    #[test]
    fn get_version_info_no_headers() {
        let headers = http::HeaderMap::new();

        let (version, _datetime) = super::get_version_info_from_headers(&headers);

        assert_eq!(None, version, "Wrong version decoded");
    }
}
