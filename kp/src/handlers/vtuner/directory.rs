use axum::{
    Router,
    extract::{OriginalUri, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

use super::{
    helpers::absolute_url_segments,
    radio_browser::{DirectoryEntry, DirectoryKind, RadioBrowserClient},
    xml::{Item, ListOfItems},
};

const POPULAR_STATION_LIMIT: usize = 200;

#[derive(Clone)]
struct DirectoryState {
    client: RadioBrowserClient,
}

pub(super) fn get_router(client: RadioBrowserClient) -> Router {
    let state = DirectoryState { client };
    Router::new()
        .route("/radiobrowser/", get(landing).post(landing))
        .route("/radiobrowser/country/", get(countries).post(countries))
        .route("/radiobrowser/language/", get(languages).post(languages))
        .route("/radiobrowser/genre/", get(genres).post(genres))
        .with_state(state)
}

async fn landing(
    State(state): State<DirectoryState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let (genres, countries, languages) = match tokio::try_join!(
        state.client.directories(DirectoryKind::Genre),
        state.client.directories(DirectoryKind::Country),
        state.client.directories(DirectoryKind::Language),
    ) {
        Ok(entries) => entries,
        Err(error) => {
            log::warn!("Radio Browser directory request failed: {error}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    let (scheme, host) = request_origin(&uri, &headers);

    render_list(ListOfItems {
        item_count: Some(4),
        items: vec![
            directory_item(
                &scheme,
                &host,
                "Genres",
                &["vtuner", "radiobrowser", "genre"],
                genres.len(),
            ),
            directory_item(
                &scheme,
                &host,
                "Countries",
                &["vtuner", "radiobrowser", "country"],
                countries.len(),
            ),
            directory_item(
                &scheme,
                &host,
                "Languages",
                &["vtuner", "radiobrowser", "language"],
                languages.len(),
            ),
            directory_item(
                &scheme,
                &host,
                "Most Popular",
                &["vtuner", "radiobrowser", "popular"],
                POPULAR_STATION_LIMIT,
            ),
        ],
        ..ListOfItems::default()
    })
}

async fn countries(
    State(state): State<DirectoryState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    list_directories(state, uri, headers, DirectoryKind::Country).await
}

async fn languages(
    State(state): State<DirectoryState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    list_directories(state, uri, headers, DirectoryKind::Language).await
}

async fn genres(
    State(state): State<DirectoryState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    list_directories(state, uri, headers, DirectoryKind::Genre).await
}

async fn list_directories(
    state: DirectoryState,
    uri: axum::http::Uri,
    headers: HeaderMap,
    kind: DirectoryKind,
) -> Response {
    let entries = match state.client.directories(kind).await {
        Ok(entries) => entries,
        Err(error) => {
            log::warn!("Radio Browser directory request failed: {error}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    if entries.is_empty() {
        return render_list(ListOfItems {
            item_count: Some(1),
            items: vec![Item::Display {
                display: "No entries found".to_owned(),
            }],
            ..ListOfItems::default()
        });
    }

    let (scheme, host) = request_origin(&uri, &headers);
    let items = entries
        .into_iter()
        .map(|entry| entry_to_item(entry, kind, &scheme, &host))
        .collect::<Vec<_>>();
    render_list(ListOfItems {
        item_count: Some(items.len()),
        items,
        ..ListOfItems::default()
    })
}

fn entry_to_item(entry: DirectoryEntry, kind: DirectoryKind, scheme: &str, host: &str) -> Item {
    let (display_name, section) = match kind {
        DirectoryKind::Country => (entry.name, "country"),
        DirectoryKind::Language => (title_case(&entry.name), "language"),
        DirectoryKind::Genre => (capitalize(&entry.name), "genre"),
    };
    let url = absolute_url_segments(
        scheme,
        host,
        &["vtuner", "radiobrowser", section, &display_name],
    );
    Item::Directory {
        title: display_name,
        url_dir: url.clone(),
        url_dir_backup: url,
        dir_count: entry.station_count as usize,
    }
}

fn directory_item(scheme: &str, host: &str, title: &str, path: &[&str], count: usize) -> Item {
    let url = absolute_url_segments(scheme, host, path);
    Item::Directory {
        title: title.to_owned(),
        url_dir: url.clone(),
        url_dir_backup: url,
        dir_count: count,
    }
}

pub(super) fn request_origin(uri: &axum::http::Uri, headers: &HeaderMap) -> (String, String) {
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .or_else(|| uri.scheme_str())
        .unwrap_or("http")
        .to_owned();
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .or_else(|| uri.authority().map(|authority| authority.as_str()))
        .unwrap_or("localhost")
        .to_owned();
    (scheme, host)
}

fn title_case(value: &str) -> String {
    value
        .split_whitespace()
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first
            .to_uppercase()
            .chain(characters.flat_map(char::to_lowercase))
            .collect(),
        None => String::new(),
    }
}

pub(super) fn render_list(list: ListOfItems) -> Response {
    match list.to_xml() {
        Ok(xml) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
            xml,
        )
            .into_response(),
        Err(error) => {
            log::error!("Failed to serialize vTuner XML: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::to_bytes, http::StatusCode};
    use tower::ServiceExt;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

    use super::*;

    const HOST: &str = "radio.example:8080";

    async fn mount_directory_mock(server: &MockServer, path: &str, body: &str) {
        Mock::given(matchers::method("GET"))
            .and(matchers::path(path))
            .and(matchers::query_param("hidebroken", "true"))
            .and(matchers::header("content-type", "application/json"))
            .and(matchers::header("user-agent", "vTuner/1.1.0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(body),
            )
            .expect(1)
            .mount(server)
            .await;
    }

    async fn response_body(response: axum::response::Response) -> String {
        String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }

    fn test_router(server: &MockServer) -> Router {
        let client = RadioBrowserClient::with_base_url(format!("{}/json/", server.uri()));
        super::super::get_router_with_client(
            &crate::configuration::VtunerConfiguration::default(),
            client,
        )
    }

    #[tokio::test]
    async fn it_filters_and_renders_country_directories_with_host_based_urls() {
        let server = MockServer::start().await;
        mount_directory_mock(
            &server,
            "/json/countries",
            r#"[
                {"name":"United States","stationcount":6},
                {"name":"Small Country","stationcount":5}
            ]"#,
        )
        .await;

        let response = test_router(&server)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/radiobrowser/country/")
                    .header("host", HOST)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::OK, response.status());
        assert_eq!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?><ListOfItems><ItemCount>1</ItemCount><Item><ItemType>Dir</ItemType><Title>United States</Title><UrlDir>http://radio.example:8080/vtuner/radiobrowser/country/United%20States/?vtuner=true</UrlDir><UrlDirBackUp>http://radio.example:8080/vtuner/radiobrowser/country/United%20States/?vtuner=true</UrlDirBackUp><DirCount>6</DirCount></Item></ListOfItems>",
            response_body(response).await
        );
    }

    #[tokio::test]
    async fn it_applies_language_and_genre_display_transforms() {
        let server = MockServer::start().await;
        mount_directory_mock(
            &server,
            "/json/languages",
            r#"[{"name":"german","stationcount":8}]"#,
        )
        .await;
        mount_directory_mock(
            &server,
            "/json/tags",
            r#"[{"name":"HIP HOP","stationcount":9}]"#,
        )
        .await;
        let router = test_router(&server);

        let language_response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/radiobrowser/language/")
                    .header("host", HOST)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let language_body = response_body(language_response).await;
        assert!(language_body.contains("<Title>German</Title>"));
        assert!(language_body.contains("/language/German/?vtuner=true"));

        let genre_response = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/radiobrowser/genre/")
                    .header("host", HOST)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let genre_body = response_body(genre_response).await;
        assert!(genre_body.contains("<Title>Hip hop</Title>"));
        assert!(genre_body.contains("/genre/Hip%20hop/?vtuner=true"));
    }

    #[tokio::test]
    async fn it_renders_radio_browser_landing_counts_and_categories() {
        let server = MockServer::start().await;
        mount_directory_mock(
            &server,
            "/json/tags",
            r#"[{"name":"pop","stationcount":10},{"name":"tiny","stationcount":4}]"#,
        )
        .await;
        mount_directory_mock(
            &server,
            "/json/countries",
            r#"[{"name":"France","stationcount":7}]"#,
        )
        .await;
        mount_directory_mock(
            &server,
            "/json/languages",
            r#"[{"name":"french","stationcount":8}]"#,
        )
        .await;

        let response = test_router(&server)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/radiobrowser/")
                    .header("host", HOST)
                    .header("x-forwarded-proto", "https")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::OK, response.status());
        let body = response_body(response).await;
        assert!(body.contains("<ItemCount>4</ItemCount>"));
        assert!(body.contains("<Title>Genres</Title>"));
        assert!(body.contains("<DirCount>1</DirCount>"));
        assert!(body.contains("<Title>Most Popular</Title>"));
        assert!(body.contains("<DirCount>200</DirCount>"));
        assert!(body.contains("https://radio.example:8080/vtuner/radiobrowser/genre/?vtuner=true"));
    }

    #[tokio::test]
    async fn it_displays_an_empty_directory_response() {
        let server = MockServer::start().await;
        mount_directory_mock(&server, "/json/tags", "[]").await;

        let response = test_router(&server)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/radiobrowser/genre/")
                    .header("host", HOST)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = response_body(response).await;
        assert!(body.contains("<ItemCount>1</ItemCount>"));
        assert!(body.contains("<Display>No entries found</Display>"));
    }
}
