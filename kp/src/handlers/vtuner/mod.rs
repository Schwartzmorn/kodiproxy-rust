mod directory;
#[allow(dead_code)]
mod helpers;
mod icon;
mod my_stations;
mod radio_browser;
mod setupapp;
mod station;
#[allow(dead_code)]
mod xml;

pub fn get_router(configuration: &crate::configuration::VtunerConfiguration) -> axum::Router {
    get_router_with_client(configuration, radio_browser::RadioBrowserClient::default())
}

fn get_router_with_client(
    configuration: &crate::configuration::VtunerConfiguration,
    client: radio_browser::RadioBrowserClient,
) -> axum::Router {
    let my_stations = my_stations::MyStations::load(configuration.stations_file.as_ref());
    let routes = axum::Router::new()
        .merge(my_stations::get_router(my_stations.clone()))
        .merge(directory::get_router(client.clone()))
        .merge(station::get_router(client.clone(), my_stations.clone()))
        .merge(icon::get_router(
            client,
            my_stations,
            configuration.cache_path.clone(),
        ));
    routes.clone().merge(setupapp::get_router(routes))
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::to_bytes,
        http::{Method, StatusCode, header},
    };
    use tower::ServiceExt;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

    #[tokio::test]
    async fn it_serves_the_setupapp_token_under_vtuner_for_get_and_post() {
        let router = Router::new().nest(
            "/vtuner",
            super::get_router(&crate::configuration::VtunerConfiguration::default()),
        );

        for method in [Method::GET, Method::POST] {
            let request = hyper::Request::builder()
                .method(method)
                .uri("/vtuner/setupapp/?token=0")
                .body(axum::body::Body::empty())
                .unwrap();
            let response = router.clone().oneshot(request).await.unwrap();

            assert_eq!(StatusCode::OK, response.status());
            assert_eq!(
                "application/xml; charset=utf-8",
                response.headers()[header::CONTENT_TYPE]
            );
            assert_eq!(
                "<EncryptedToken>0000000000000000</EncryptedToken>",
                std::str::from_utf8(
                    to_bytes(response.into_body(), usize::MAX)
                        .await
                        .unwrap()
                        .as_ref(),
                )
                .unwrap()
            );
        }
    }

    #[tokio::test]
    async fn it_serves_the_landing_page_under_the_vtuner_prefix_for_get_and_post() {
        let router = Router::new().nest(
            "/vtuner",
            super::get_router(&crate::configuration::VtunerConfiguration::default()),
        );

        for method in [Method::GET, Method::POST] {
            let response = router
                .clone()
                .oneshot(
                    hyper::Request::builder()
                        .method(method.clone())
                        .uri("/vtuner")
                        .header("host", "radio.example:8080")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(StatusCode::OK, response.status(), "{method}");
            let xml = String::from_utf8(
                to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();
            assert!(xml.contains("<ItemCount>1</ItemCount>"));
            assert!(xml.contains("http://radio.example:8080/vtuner/radiobrowser/?vtuner=true"));
            assert!(xml.contains("<Display>'My Stations' feature not configured.</Display>"));
        }
    }

    #[tokio::test]
    async fn it_dispatches_setupapp_navigation_to_the_radio_browser_landing() {
        let server = MockServer::start().await;
        for (path, body) in [
            ("/json/tags", r#"[{"name":"pop","stationcount":10}]"#),
            ("/json/countries", r#"[{"name":"France","stationcount":7}]"#),
            ("/json/languages", r#"[{"name":"french","stationcount":8}]"#),
        ] {
            Mock::given(matchers::method("GET"))
                .and(matchers::path(path))
                .and(matchers::query_param("hidebroken", "true"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "application/json")
                        .set_body_string(body),
                )
                .expect(1)
                .mount(&server)
                .await;
        }

        let client = super::radio_browser::RadioBrowserClient::with_base_url(format!(
            "{}/json/",
            server.uri()
        ));
        let router = Router::new().nest(
            "/vtuner",
            super::get_router_with_client(
                &crate::configuration::VtunerConfiguration::default(),
                client,
            ),
        );
        let response = router
            .oneshot(
                hyper::Request::builder()
                    .uri("/vtuner/setupapp/navXML.asp")
                    .header("host", "radio.example:8080")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());
        let xml = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(xml.contains("<Title>Genres</Title>"));
        assert!(xml.contains("<Title>Countries</Title>"));
        assert!(xml.contains("<Title>Languages</Title>"));
        assert!(xml.contains("<Title>Most Popular</Title>"));
    }
}
