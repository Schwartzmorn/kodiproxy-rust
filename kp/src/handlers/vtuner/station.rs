use std::collections::HashMap;

use axum::{
    Router,
    extract::{OriginalUri, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

use super::{
    directory::{render_list, request_origin},
    helpers::{
        absolute_url_segments, page_bounds, rewrite_https_to_http, station_id_prefix,
        station_id_without_prefix,
    },
    my_stations::MyStations,
    radio_browser::{RadioBrowserClient, RadioBrowserStation},
    xml::{Item, ListOfItems},
};

#[derive(Clone)]
struct StationState {
    client: RadioBrowserClient,
    my_stations: Option<MyStations>,
}

pub(super) fn get_router(client: RadioBrowserClient, my_stations: Option<MyStations>) -> Router {
    Router::new()
        .route(
            "/radiobrowser/country/{directory}/",
            get(country_stations).post(country_stations),
        )
        .route(
            "/radiobrowser/language/{directory}/",
            get(language_stations).post(language_stations),
        )
        .route(
            "/radiobrowser/genre/{directory}/",
            get(genre_stations).post(genre_stations),
        )
        .route("/radiobrowser/popular/", get(popular).post(popular))
        .route("/search/", get(search).post(search))
        .route("/station", get(station_info).post(station_info))
        .route("/play", get(play).post(play))
        .with_state(StationState {
            client,
            my_stations,
        })
}

async fn country_stations(
    State(state): State<StationState>,
    Path(directory): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let result = state.client.stations_by_country(&directory).await;
    station_list_response(result, uri, headers, parameters)
}

async fn language_stations(
    State(state): State<StationState>,
    Path(directory): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let result = state.client.stations_by_language(&directory).await;
    station_list_response(result, uri, headers, parameters)
}

async fn genre_stations(
    State(state): State<StationState>,
    Path(directory): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let result = state.client.stations_by_genre(&directory).await;
    station_list_response(result, uri, headers, parameters)
}

async fn popular(
    State(state): State<StationState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let result = state.client.popular_stations().await;
    station_list_response(result, uri, headers, parameters)
}

async fn search(
    State(state): State<StationState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let search_term = parameters
        .get("search")
        .map(String::as_str)
        .unwrap_or_default();
    if search_term.chars().count() < 3 {
        return render_list(ListOfItems {
            item_count: Some(1),
            items: vec![Item::Display {
                display: "Search query too short".to_owned(),
            }],
            ..ListOfItems::default()
        });
    }

    let result = state.client.search_stations(search_term).await;
    station_list_response(result, uri, headers, parameters)
}

fn station_list_response(
    result: Result<Vec<RadioBrowserStation>, reqwest::Error>,
    uri: axum::http::Uri,
    headers: HeaderMap,
    parameters: HashMap<String, String>,
) -> Response {
    let stations = match result {
        Ok(stations) => stations
            .into_iter()
            .filter(|station| station.lastcheckok == Some(1))
            .collect::<Vec<_>>(),
        Err(error) => {
            log::warn!("Radio Browser station request failed: {error}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    if stations.is_empty() {
        return render_list(ListOfItems {
            item_count: Some(1),
            items: vec![Item::Display {
                display: "No stations found".to_owned(),
            }],
            ..ListOfItems::default()
        });
    }

    let total_count = stations.len();
    let page = page_bounds(&parameters, total_count);
    let (scheme, host) = request_origin(&uri, &headers);
    let items = stations[page]
        .iter()
        .map(|station| station_to_item(station, &scheme, &host, None))
        .collect();
    render_list(ListOfItems {
        item_count: Some(total_count),
        items,
        ..ListOfItems::default()
    })
}

async fn station_info(
    State(state): State<StationState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let Some(station_id) = parameters.get("id").filter(|id| !id.is_empty()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let (scheme, host) = request_origin(&uri, &headers);

    if matches!(station_id_prefix(station_id), Some("MY")) {
        let Some(station) = state
            .my_stations
            .as_ref()
            .and_then(|stations| stations.station_by_id(station_id))
        else {
            return render_list(ListOfItems {
                item_count: Some(1),
                items: vec![Item::Display {
                    display: "Station not found".to_owned(),
                }],
                ..ListOfItems::default()
            });
        };
        return render_list(ListOfItems {
            item_count: Some(1),
            items: vec![my_station_to_item(station, &scheme, &host)],
            ..ListOfItems::default()
        });
    }

    let station = match get_radio_browser_station(&state.client, station_id).await {
        Ok(Some(station)) => station,
        Ok(None) => {
            return render_list(ListOfItems {
                item_count: Some(1),
                items: vec![Item::Display {
                    display: "Station not found".to_owned(),
                }],
                ..ListOfItems::default()
            });
        }
        Err(error) => {
            log::warn!("Radio Browser station lookup failed: {error}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let playable_url = match state.client.playable_url(&station.stationuuid).await {
        Ok(url) => url,
        Err(error) => {
            log::warn!("Radio Browser playable URL lookup failed: {error}");
            None
        }
    };
    render_list(ListOfItems {
        item_count: Some(1),
        items: vec![station_to_item(&station, &scheme, &host, playable_url)],
        ..ListOfItems::default()
    })
}

async fn play(
    State(state): State<StationState>,
    Query(parameters): Query<HashMap<String, String>>,
) -> Response {
    let Some(station_id) = parameters.get("id").filter(|id| !id.is_empty()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    if matches!(station_id_prefix(station_id), Some("MY")) {
        let Some(station) = state
            .my_stations
            .as_ref()
            .and_then(|stations| stations.station_by_id(station_id))
        else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let stream_url = rewrite_https_to_http(&station.url);
        return match HeaderValue::from_str(&stream_url) {
            Ok(location) => (StatusCode::FOUND, [(header::LOCATION, location)]).into_response(),
            Err(_) => StatusCode::BAD_GATEWAY.into_response(),
        };
    }

    let station = match get_radio_browser_station(&state.client, station_id).await {
        Ok(Some(station)) => station,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            log::warn!("Radio Browser station lookup failed: {error}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let stream_url = match state.client.playable_url(&station.stationuuid).await {
        Ok(Some(url)) => url,
        Ok(None) => station.url,
        Err(error) => {
            log::warn!("Radio Browser playable URL lookup failed: {error}");
            station.url
        }
    };
    let stream_url = rewrite_https_to_http(&stream_url);
    match HeaderValue::from_str(&stream_url) {
        Ok(location) => (StatusCode::FOUND, [(header::LOCATION, location)]).into_response(),
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}

async fn get_radio_browser_station(
    client: &RadioBrowserClient,
    station_id: &str,
) -> Result<Option<RadioBrowserStation>, reqwest::Error> {
    if station_id_prefix(station_id) != Some("RB") {
        return Ok(None);
    }
    let Some(station_uuid) = station_id_without_prefix(station_id) else {
        return Ok(None);
    };
    client.station_by_id(station_uuid).await
}

fn station_to_item(
    station: &RadioBrowserStation,
    scheme: &str,
    host: &str,
    playable_url: Option<String>,
) -> Item {
    let tags = station
        .tags
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();
    let station_id = format!("RB_{}", station.stationuuid);
    let icon_url = absolute_url_segments(scheme, host, &["vtuner", "icon"]);
    let mut query = form_urlencoded::Serializer::new(String::new());
    query.append_pair("id", &station_id);

    Item::Station {
        station_id,
        station_name: station.name.clone(),
        station_url: playable_url.unwrap_or_else(|| station.url.clone()),
        station_desc: tags.join(", "),
        logo: format!("{}?{}", icon_url.trim_end_matches('/'), query.finish()),
        station_format: tags.first().copied().unwrap_or_default().to_owned(),
        station_location: station.countrycode.clone(),
        station_bandwidth: station.bitrate.map(|bitrate| bitrate.to_string()),
        station_mime: station.codec.clone(),
        relia: "3",
        bookmark: String::new(),
    }
}

fn my_station_to_item(station: &super::my_stations::MyStation, scheme: &str, host: &str) -> Item {
    let icon_url = absolute_url_segments(scheme, host, &["vtuner", "icon"]);
    let mut query = form_urlencoded::Serializer::new(String::new());
    query.append_pair("id", &station.id);

    Item::Station {
        station_id: station.id.clone(),
        station_name: station.name.clone(),
        station_url: rewrite_https_to_http(&station.url),
        station_desc: station.category.clone(),
        logo: format!("{}?{}", icon_url.trim_end_matches('/'), query.finish()),
        station_format: station.category.clone(),
        station_location: None,
        station_bandwidth: None,
        station_mime: None,
        relia: "3",
        bookmark: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::to_bytes, http::StatusCode};
    use tower::ServiceExt;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

    use super::*;

    const HOST: &str = "radio.example:8080";
    const UUID: &str = "uuid-1";

    fn station_json(lastcheckok: i32, name: &str, url: &str) -> String {
        format!(
            r#"{{"stationuuid":"{UUID}","name":"{name}","url":"{url}","favicon":"https://images.example/icon.png","tags":"rock, indie","countrycode":"DE","codec":"MP3","bitrate":128,"lastcheckok":{lastcheckok}}}"#
        )
    }

    async fn mount_json(server: &MockServer, path: &str, query: &[(&str, &str)], body: &str) {
        let mut mock = Mock::given(matchers::method("GET")).and(matchers::path(path));
        for (name, value) in query {
            mock = mock.and(matchers::query_param(*name, *value));
        }
        mock.and(matchers::header("user-agent", "vTuner/1.1.0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(body),
            )
            .expect(1)
            .mount(server)
            .await;
    }

    async fn mount_json_twice(server: &MockServer, path: &str, body: &str) {
        Mock::given(matchers::method("GET"))
            .and(matchers::path(path))
            .and(matchers::header("user-agent", "vTuner/1.1.0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(body),
            )
            .expect(2)
            .mount(server)
            .await;
    }

    fn test_router(server: &MockServer) -> Router {
        let client = RadioBrowserClient::with_base_url(format!("{}/json/", server.uri()));
        super::super::get_router_with_client(
            &crate::configuration::VtunerConfiguration::default(),
            client,
        )
    }

    async fn body(response: axum::response::Response) -> String {
        String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn it_lists_only_playable_country_stations_and_applies_paging() {
        let server = MockServer::start().await;
        let stations = format!(
            "[{},{}]",
            station_json(1, "A Radio", "https://a.example"),
            station_json(0, "Broken Radio", "http://broken.example")
        );
        mount_json(
            &server,
            "/json/stations/search",
            &[
                ("order", "name"),
                ("reverse", "false"),
                ("countryExact", "true"),
                ("country", "Germany"),
            ],
            &stations,
        )
        .await;

        let response = test_router(&server)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/radiobrowser/country/Germany/?startitems=1&enditems=10")
                    .header("host", HOST)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::OK, response.status());
        let xml = body(response).await;
        assert!(xml.contains("<ItemCount>1</ItemCount>"));
        assert!(xml.contains("<StationId>RB_uuid-1</StationId>"));
        assert!(xml.contains("<StationUrl>http://a.example</StationUrl>"));
        assert!(xml.contains("<StationDesc>rock, indie</StationDesc>"));
        assert!(xml.contains("<StationFormat>rock</StationFormat>"));
        assert!(xml.contains("<StationLocation>DE</StationLocation>"));
        assert!(xml.contains("<StationBandWidth>128</StationBandWidth>"));
        assert!(xml.contains("<StationMime>MP3</StationMime>"));
        assert!(xml.contains("http://radio.example:8080/vtuner/icon?id=RB_uuid-1"));
        assert!(!xml.contains("Broken Radio"));
    }

    #[tokio::test]
    async fn it_calls_each_exact_station_filter_endpoint() {
        let server = MockServer::start().await;
        let empty = "[]";
        mount_json(
            &server,
            "/json/stations/search",
            &[
                ("order", "name"),
                ("reverse", "false"),
                ("languageExact", "true"),
                ("language", "German"),
            ],
            empty,
        )
        .await;
        mount_json(
            &server,
            "/json/stations/search",
            &[
                ("order", "name"),
                ("reverse", "false"),
                ("tagExact", "true"),
                ("tag", "Jazz"),
            ],
            empty,
        )
        .await;
        mount_json(
            &server,
            "/json/stations",
            &[("order", "votes"), ("reverse", "true"), ("limit", "200")],
            empty,
        )
        .await;
        let router = test_router(&server);
        for path in [
            "/radiobrowser/language/German/",
            "/radiobrowser/genre/Jazz/",
            "/radiobrowser/popular/",
        ] {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(path)
                        .header("host", HOST)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(
                body(response)
                    .await
                    .contains("<Display>No stations found</Display>")
            );
        }
    }

    #[tokio::test]
    async fn it_renders_nonempty_language_genre_and_popular_station_lists() {
        let server = MockServer::start().await;
        let stations = format!(
            "[{},{}]",
            station_json(1, "Available Radio", "https://stream.example/available"),
            station_json(0, "Broken Radio", "http://stream.example/broken")
        );
        mount_json(
            &server,
            "/json/stations/search",
            &[
                ("order", "name"),
                ("reverse", "false"),
                ("languageExact", "true"),
                ("language", "German"),
            ],
            &stations,
        )
        .await;
        mount_json(
            &server,
            "/json/stations/search",
            &[
                ("order", "name"),
                ("reverse", "false"),
                ("tagExact", "true"),
                ("tag", "Jazz"),
            ],
            &stations,
        )
        .await;
        mount_json(
            &server,
            "/json/stations",
            &[("order", "votes"), ("reverse", "true"), ("limit", "200")],
            &stations,
        )
        .await;

        let router = test_router(&server);
        for path in [
            "/radiobrowser/language/German/",
            "/radiobrowser/genre/Jazz/",
            "/radiobrowser/popular/",
        ] {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(path)
                        .header("host", HOST)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(StatusCode::OK, response.status(), "{path}");
            let xml = body(response).await;
            assert!(xml.contains("<ItemCount>1</ItemCount>"), "{path}");
            assert!(
                xml.contains("<StationName>Available Radio</StationName>"),
                "{path}"
            );
            assert!(
                xml.contains("<StationUrl>http://stream.example/available</StationUrl>"),
                "{path}"
            );
            assert!(!xml.contains("Broken Radio"), "{path}");
        }
    }

    #[tokio::test]
    async fn it_rejects_short_search_and_queries_radio_browser_for_valid_search() {
        let server = MockServer::start().await;
        mount_json(
            &server,
            "/json/stations/search",
            &[
                ("order", "name"),
                ("reverse", "false"),
                ("limit", "200"),
                ("name", "deep house"),
            ],
            &format!(
                "[{}]",
                station_json(1, "Deep House", "http://stream.example")
            ),
        )
        .await;
        mount_json(
            &server,
            "/json/stations/search",
            &[
                ("order", "name"),
                ("reverse", "false"),
                ("limit", "200"),
                ("name", "missing station"),
            ],
            "[]",
        )
        .await;
        let router = test_router(&server);
        for path in ["/search/?search=ab", "/search/?search=", "/search/"] {
            let short = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(path)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(
                body(short)
                    .await
                    .contains("<Display>Search query too short</Display>"),
                "{path}"
            );
        }

        let valid = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/search/?search=deep%20house")
                    .header("host", HOST)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            body(valid)
                .await
                .contains("<StationName>Deep House</StationName>")
        );

        let empty = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/search/?search=missing%20station")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            body(empty)
                .await
                .contains("<Display>No stations found</Display>")
        );
    }

    #[tokio::test]
    async fn it_returns_resolved_play_url_and_station_details_even_for_broken_station() {
        let server = MockServer::start().await;
        mount_json_twice(
            &server,
            "/json/stations/byuuid/uuid-1",
            &format!(
                "[{}]",
                station_json(0, "Broken Radio", "http://original.example/stream")
            ),
        )
        .await;
        mount_json_twice(
            &server,
            "/json/url/uuid-1",
            r#"[{"url":"https://resolved.example/stream"}]"#,
        )
        .await;
        let router = test_router(&server);

        let details = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/station?id=RB_uuid-1")
                    .header("host", HOST)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let details_xml = body(details).await;
        assert!(details_xml.contains("<StationId>RB_uuid-1</StationId>"));
        assert!(details_xml.contains("<StationName>Broken Radio</StationName>"));
        assert!(details_xml.contains("<StationUrl>http://resolved.example/stream</StationUrl>"));
        assert!(details_xml.contains("<StationDesc>rock, indie</StationDesc>"));
        assert!(details_xml.contains("<StationFormat>rock</StationFormat>"));
        assert!(details_xml.contains("<StationLocation>DE</StationLocation>"));
        assert!(details_xml.contains("<StationBandWidth>128</StationBandWidth>"));
        assert!(details_xml.contains("<StationMime>MP3</StationMime>"));
        assert!(
            details_xml.contains("<Logo>http://radio.example:8080/vtuner/icon?id=RB_uuid-1</Logo>")
        );

        let play = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/play?id=RB_uuid-1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::FOUND, play.status());
        assert_eq!(
            "http://resolved.example/stream",
            play.headers()[header::LOCATION]
        );
    }

    #[tokio::test]
    async fn it_falls_back_to_the_original_stream_url_when_no_playable_url_is_returned() {
        let server = MockServer::start().await;
        mount_json(
            &server,
            "/json/stations/byuuid/uuid-1",
            &[],
            &format!(
                "[{}]",
                station_json(1, "Test Radio", "https://original.example/stream")
            ),
        )
        .await;
        mount_json(&server, "/json/url/uuid-1", &[], "[]").await;

        let response = test_router(&server)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/play?id=RB_uuid-1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::FOUND, response.status());
        assert_eq!(
            "http://original.example/stream",
            response.headers()[header::LOCATION]
        );
    }

    #[tokio::test]
    async fn it_handles_missing_and_unknown_station_ids() {
        let server = MockServer::start().await;
        mount_json(&server, "/json/stations/byuuid/unknown", &[], "[]").await;
        let router = test_router(&server);

        let missing = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/station")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::BAD_REQUEST, missing.status());

        let unknown_info = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/station?id=RB_unknown")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::OK, unknown_info.status());
        assert!(
            body(unknown_info)
                .await
                .contains("<Display>Station not found</Display>")
        );

        let unknown_play = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/play?id=MY_unknown")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::NOT_FOUND, unknown_play.status());
    }

    #[tokio::test]
    async fn it_uses_my_station_ids_for_station_info_and_play() {
        let path = std::env::temp_dir().join(format!(
            "vtuner-my-station-route-{}-{}.yml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "Local:\n  Jazz Club: http://example.com/jazz\n").unwrap();

        let config = crate::configuration::VtunerConfiguration {
            stations_file: Some(path.clone()),
            ..crate::configuration::VtunerConfiguration::default()
        };
        let router = super::super::get_router_with_client(
            &config,
            RadioBrowserClient::with_base_url("http://radio.example:8080/json/"),
        );
        let station_id = super::super::my_stations::MyStations::load(Some(&path))
            .unwrap()
            .category_stations("Local")
            .first()
            .unwrap()
            .id
            .clone();

        let info = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/station?id={station_id}"))
                    .header("host", HOST)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::OK, info.status());
        let info_xml = body(info).await;
        assert!(info_xml.contains("<StationName>Jazz Club</StationName>"));
        assert!(info_xml.contains("<StationUrl>http://example.com/jazz</StationUrl>"));

        let play = router
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/play?id={station_id}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::FOUND, play.status());
        assert_eq!("http://example.com/jazz", play.headers()[header::LOCATION]);

        std::fs::remove_file(path).unwrap();
    }
}
