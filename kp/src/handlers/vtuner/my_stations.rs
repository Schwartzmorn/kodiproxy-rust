use std::collections::BTreeMap;

use axum::{
    Router,
    extract::{OriginalUri, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};

use super::{
    directory::{render_list, request_origin},
    helpers::{absolute_url_segments, my_station_id, rewrite_https_to_http},
    xml::{Item, ListOfItems},
};

#[derive(Debug, Clone, Default)]
pub(super) struct MyStations {
    categories: BTreeMap<String, Vec<MyStation>>,
}

#[derive(Debug, Clone)]
pub(super) struct MyStation {
    pub id: String,
    pub name: String,
    pub url: String,
    pub category: String,
}

impl MyStations {
    pub fn load(path: Option<&std::path::PathBuf>) -> Option<Self> {
        let path = path?;
        let file = std::fs::read_to_string(path).ok()?;
        let parsed: BTreeMap<String, BTreeMap<String, String>> =
            serde_yaml::from_str(&file).ok()?;

        let categories = parsed
            .into_iter()
            .map(|(category, stations)| {
                let values = stations
                    .into_iter()
                    .map(|(name, url)| MyStation {
                        id: my_station_id(&name, &url),
                        name,
                        url,
                        category: category.clone(),
                    })
                    .collect::<Vec<_>>();
                (category, values)
            })
            .collect();

        Some(Self { categories })
    }

    pub fn is_enabled(&self) -> bool {
        !self.categories.is_empty()
    }

    pub fn categories(&self) -> Vec<(&str, usize)> {
        self.categories
            .iter()
            .map(|(category, stations)| (category.as_str(), stations.len()))
            .collect()
    }

    pub fn category_stations(&self, category: &str) -> Vec<&MyStation> {
        self.categories
            .get(category)
            .map(|stations| stations.iter().collect())
            .unwrap_or_default()
    }

    pub fn station_by_id(&self, station_id: &str) -> Option<&MyStation> {
        self.categories
            .values()
            .flatten()
            .find(|station| station.id == station_id)
    }
}

#[derive(Clone)]
struct MyStationsState {
    stations: Option<MyStations>,
}

pub(super) fn get_router(stations: Option<MyStations>) -> Router {
    Router::new()
        .route("/", get(vtuner_root).post(vtuner_root))
        .route(
            "/my_stations/",
            get(my_stations_landing).post(my_stations_landing),
        )
        .route(
            "/my_stations/{directory}/",
            get(my_stations_category).post(my_stations_category),
        )
        .with_state(MyStationsState { stations })
}

async fn vtuner_root(
    State(state): State<MyStationsState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let (scheme, host) = request_origin(&uri, &headers);
    let mut items = vec![directory_item(
        &scheme,
        &host,
        "Radiobrowser",
        &["vtuner", "radiobrowser"],
        4,
    )];

    match state.stations {
        Some(stations) if stations.is_enabled() => {
            items.push(directory_item(
                &scheme,
                &host,
                "My Stations",
                &["vtuner", "my_stations"],
                stations.categories().len(),
            ));
            render_list(ListOfItems {
                item_count: None,
                items,
                ..ListOfItems::default()
            })
        }
        Some(_) | None => {
            items.push(Item::Display {
                display: "'My Stations' feature not configured.".to_owned(),
            });
            render_list(ListOfItems {
                item_count: Some(1),
                items,
                ..ListOfItems::default()
            })
        }
    }
}

async fn my_stations_landing(
    State(state): State<MyStationsState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let Some(stations) = state.stations else {
        return render_list(ListOfItems {
            item_count: Some(1),
            items: vec![Item::Display {
                display: "'My Stations' feature not configured.".to_owned(),
            }],
            ..ListOfItems::default()
        });
    };

    let categories = stations.categories();
    if categories.is_empty() {
        return render_list(ListOfItems {
            item_count: Some(1),
            items: vec![Item::Display {
                display: "No entries found".to_owned(),
            }],
            ..ListOfItems::default()
        });
    }

    let (scheme, host) = request_origin(&uri, &headers);
    let items: Vec<Item> = categories
        .into_iter()
        .map(|(category, count)| {
            let url = absolute_url_segments(
                scheme.as_str(),
                host.as_str(),
                &["vtuner", "my_stations", category],
            );
            Item::Directory {
                title: category.to_owned(),
                url_dir: url.clone(),
                url_dir_backup: url,
                dir_count: count,
            }
        })
        .collect();

    render_list(ListOfItems {
        item_count: Some(items.len()),
        items,
        ..ListOfItems::default()
    })
}

async fn my_stations_category(
    State(state): State<MyStationsState>,
    Path(directory): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let Some(stations) = state.stations else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let station_entries = stations.category_stations(&directory);
    if station_entries.is_empty() {
        return render_list(ListOfItems {
            item_count: Some(1),
            items: vec![Item::Display {
                display: "No stations found".to_owned(),
            }],
            ..ListOfItems::default()
        });
    }

    let (scheme, host) = request_origin(&uri, &headers);
    let items: Vec<Item> = station_entries
        .into_iter()
        .map(|station| my_station_to_item(station, &scheme, &host))
        .collect();

    render_list(ListOfItems {
        item_count: Some(items.len()),
        items,
        ..ListOfItems::default()
    })
}

fn my_station_to_item(station: &MyStation, scheme: &str, host: &str) -> Item {
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

fn directory_item(scheme: &str, host: &str, title: &str, path: &[&str], count: usize) -> Item {
    let url = absolute_url_segments(scheme, host, path);
    Item::Directory {
        title: title.to_owned(),
        url_dir: url.clone(),
        url_dir_backup: url,
        dir_count: count,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use axum::{body::to_bytes, http::StatusCode};
    use tower::ServiceExt;

    use super::*;

    fn temp_yaml(contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vtuner-my-stations-{}-{}.yml",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, contents).unwrap();
        path
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

    #[test]
    fn it_loads_and_maps_my_stations_from_yaml() {
        let path = temp_yaml(
            "Local:\n  Jazz Club: http://example.com/jazz\n  Lounge: https://example.com/lounge\n",
        );
        let stations = MyStations::load(Some(&path)).unwrap();

        assert!(stations.is_enabled());
        assert_eq!(1, stations.categories().len());
        assert_eq!(2, stations.category_stations("Local").len());

        let lounge_id = my_station_id("Lounge", "https://example.com/lounge");
        assert_eq!("Lounge", stations.station_by_id(&lounge_id).unwrap().name);
        assert_eq!(
            "Local",
            stations.station_by_id(&lounge_id).unwrap().category
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn it_returns_none_for_missing_or_invalid_yaml() {
        let missing = std::env::temp_dir().join("definitely-missing-vtuner-stations.yml");
        assert!(MyStations::load(Some(&missing)).is_none());

        let invalid = temp_yaml("Category: [\n");
        assert!(MyStations::load(Some(&invalid)).is_none());
        fs::remove_file(invalid).unwrap();
    }

    #[tokio::test]
    async fn it_routes_my_stations_landing_and_category_views() {
        let path = temp_yaml(
            "Local:\n  Jazz Club: http://example.com/jazz\nFavorite:\n  Chill House: http://example.com/chill\n",
        );
        let router = super::get_router(MyStations::load(Some(&path)));

        let landing = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/my_stations/")
                    .header("host", "radio.example:8080")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::OK, landing.status());
        let landing_xml = body(landing).await;
        assert!(landing_xml.contains("<ItemCount>2</ItemCount>"));
        assert!(landing_xml.contains("<Title>Favorite</Title>"));
        assert!(landing_xml.contains("<Title>Local</Title>"));
        assert!(landing_xml.contains("/vtuner/my_stations/Local/?vtuner=true"));

        let category = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/my_stations/Local/")
                    .header("host", "radio.example:8080")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::OK, category.status());
        let category_xml = body(category).await;
        assert!(category_xml.contains("<ItemCount>1</ItemCount>"));
        assert!(category_xml.contains("<StationId>MY_"));
        assert!(category_xml.contains("<StationName>Jazz Club</StationName>"));
        assert!(category_xml.contains("<StationUrl>http://example.com/jazz</StationUrl>"));
        assert!(category_xml.contains("<StationDesc>Local</StationDesc>"));
        assert!(category_xml.contains("<StationFormat>Local</StationFormat>"));
        assert!(category_xml.contains("<Logo>http://radio.example:8080/vtuner/icon?id=MY_"));
        assert!(category_xml.contains("<Relia>3</Relia>"));

        let unknown_category = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/my_stations/Unknown/")
                    .header("host", "radio.example:8080")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(StatusCode::OK, unknown_category.status());
        assert!(
            body(unknown_category)
                .await
                .contains("<Display>No stations found</Display>")
        );

        fs::remove_file(path).unwrap();
    }
}
