use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

use super::{
    helpers::{station_id_prefix, station_id_without_prefix},
    my_stations::MyStations,
    radio_browser::RadioBrowserClient,
};

const ICON_SIZE: u32 = 290;

#[derive(Clone)]
struct IconState {
    client: RadioBrowserClient,
    my_stations: Option<MyStations>,
    cache_path: PathBuf,
    http: reqwest::Client,
}

pub(super) fn get_router(
    client: RadioBrowserClient,
    my_stations: Option<MyStations>,
    cache_path: PathBuf,
) -> Router {
    Router::new()
        .route("/icon", get(icon).post(icon))
        .with_state(IconState {
            client,
            my_stations,
            cache_path,
            http: crate::reqwest_client(),
        })
}

async fn icon(
    State(state): State<IconState>,
    Query(parameters): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let Some(station_id) = parameters.get("id").filter(|id| !id.is_empty()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !valid_station_id(station_id) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let cache_file = state.cache_path.join("icons").join(station_id);
    match tokio::fs::read(&cache_file).await {
        Ok(contents) => return jpeg_response(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            log::warn!("Unable to read vTuner icon cache: {error}");
            return StatusCode::NOT_FOUND.into_response();
        }
    }

    let Some(favicon_url) = find_favicon(&state, station_id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let image_data = match state.http.get(favicon_url).send().await {
        Ok(response) if response.status().is_success() => match response.bytes().await {
            Ok(bytes) => bytes,
            Err(error) => {
                log::warn!("Unable to read station icon response: {error}");
                return StatusCode::NOT_FOUND.into_response();
            }
        },
        Ok(response) => {
            log::debug!("Station icon request returned {}", response.status());
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(error) => {
            log::debug!("Station icon request failed: {error}");
            return StatusCode::NOT_FOUND.into_response();
        }
    };

    let jpeg = match convert_to_jpeg(&image_data) {
        Ok(jpeg) => jpeg,
        Err(error) => {
            log::debug!("Unable to convert station icon: {error}");
            return StatusCode::NOT_FOUND.into_response();
        }
    };

    if let Err(error) = cache_icon(&cache_file, &jpeg).await {
        log::warn!("Unable to write vTuner icon cache: {error}");
        return StatusCode::NOT_FOUND.into_response();
    }

    jpeg_response(jpeg)
}

async fn find_favicon(state: &IconState, station_id: &str) -> Option<String> {
    match station_id_prefix(station_id) {
        Some("RB") => {
            let station_uuid = station_id_without_prefix(station_id)?;
            match state.client.station_icon_url(station_uuid).await {
                Ok(url) => url,
                Err(error) => {
                    log::debug!("Radio Browser station icon lookup failed: {error}");
                    None
                }
            }
        }
        Some("MY") => {
            state
                .my_stations
                .as_ref()
                .and_then(|stations| stations.station_by_id(station_id))?;
            None
        }
        _ => None,
    }
}

fn valid_station_id(station_id: &str) -> bool {
    station_id_prefix(station_id).is_some()
        && station_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn convert_to_jpeg(image_data: &[u8]) -> Result<Vec<u8>, image::ImageError> {
    let image = image::ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()?
        .decode()?;
    let image = image.thumbnail(ICON_SIZE, ICON_SIZE).to_rgb8();
    let mut output = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image).write_to(&mut output, image::ImageFormat::Jpeg)?;
    Ok(output.into_inner())
}

async fn cache_icon(cache_file: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = cache_file
        .parent()
        .expect("icon cache filename has a parent directory");
    tokio::fs::create_dir_all(parent).await?;
    tokio::fs::write(cache_file, contents).await
}

fn jpeg_response(contents: impl Into<Bytes>) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/jpeg")],
        contents.into(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use axum::{body::to_bytes, http::StatusCode};
    use image::{ImageFormat, RgbImage};
    use tower::ServiceExt;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

    use super::*;

    fn temporary_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vtuner-icons-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn png_image(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_pixel(width, height, image::Rgb([30, 90, 150]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn router(server: &MockServer, cache_path: PathBuf) -> Router {
        let client = RadioBrowserClient::with_base_url(format!("{}/json/", server.uri()));
        get_router(client, None, cache_path)
    }

    async fn mount_station(server: &MockServer, favicon_url: &str) {
        Mock::given(matchers::method("GET"))
            .and(matchers::path("/json/stations/byuuid/uuid-1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(format!(
                        r#"[{{"stationuuid":"uuid-1","name":"Radio","url":"http://stream.example","favicon":"{favicon_url}","tags":"","countrycode":"","codec":"","bitrate":0,"lastcheckok":1}}]"#
                    )),
            )
            .expect(1)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn it_fetches_resizes_encodes_and_caches_station_icons() {
        let server = MockServer::start().await;
        let png = png_image(600, 300);
        let bytes = png.clone();
        Mock::given(matchers::method("GET"))
            .and(matchers::path("/icon.png"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/png")
                    .set_body_bytes(bytes),
            )
            .expect(1)
            .mount(&server)
            .await;
        mount_station(&server, &format!("{}/icon.png", server.uri())).await;

        let cache_path = temporary_directory();
        let router = router(&server, cache_path.clone());
        for _ in 0..2 {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/icon?id=RB_uuid-1")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(StatusCode::OK, response.status());
            assert_eq!("image/jpeg", response.headers()[header::CONTENT_TYPE]);
            let contents = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let image = image::load_from_memory_with_format(&contents, ImageFormat::Jpeg).unwrap();
            assert_eq!((290, 145), (image.width(), image.height()));
        }

        assert!(cache_path.join("icons/RB_uuid-1").exists());
        assert!(!png.is_empty());
        fs::remove_dir_all(cache_path).unwrap();
    }

    #[tokio::test]
    async fn it_returns_not_found_for_missing_icons_and_bad_ids() {
        let server = MockServer::start().await;
        mount_station(&server, "").await;
        let cache_path = temporary_directory();
        let router = router(&server, cache_path.clone());

        for uri in ["/icon", "/icon?id=../secret", "/icon?id=RB_uuid-1"] {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(uri)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let expected = if uri == "/icon" {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NOT_FOUND
            };
            assert_eq!(expected, response.status(), "{uri}");
        }
        fs::remove_dir_all(cache_path).unwrap();
    }

    #[tokio::test]
    async fn it_returns_not_found_when_fetch_or_image_conversion_fails() {
        for (favicon_path, status, contents) in [
            ("/unavailable.png", 503, Vec::new()),
            ("/invalid.png", 200, b"not an image".to_vec()),
        ] {
            let server = MockServer::start().await;
            Mock::given(matchers::method("GET"))
                .and(matchers::path(favicon_path))
                .respond_with(ResponseTemplate::new(status).set_body_bytes(contents))
                .expect(1)
                .mount(&server)
                .await;
            mount_station(&server, &format!("{}{favicon_path}", server.uri())).await;
            let cache_path = temporary_directory();
            let response = router(&server, cache_path.clone())
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/icon?id=RB_uuid-1")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(StatusCode::NOT_FOUND, response.status());
            fs::remove_dir_all(cache_path).unwrap();
        }
    }

    #[tokio::test]
    async fn it_returns_not_found_when_the_cache_path_is_not_a_directory() {
        let server = MockServer::start().await;
        let cache_path = temporary_directory().join("cache-file");
        fs::write(&cache_path, "not a directory").unwrap();
        let response = router(&server, cache_path.clone())
            .oneshot(
                axum::http::Request::builder()
                    .uri("/icon?id=RB_uuid-1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());
        fs::remove_file(&cache_path).unwrap();
        fs::remove_dir(cache_path.parent().unwrap()).unwrap();
    }
}
