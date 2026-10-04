use axum::{
    Json, Router,
    extract::{OriginalUri, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::timeout::TimeoutLayer;

type AVReceiver = std::sync::Arc<dyn crate::avreceiver::AVReceiverInterface>;

pub fn add_routes(router: Router, receiver: AVReceiver) -> Router {
    let avreceiver_router = Router::new()
        .route("/volume", get(handle_volume))
        .route("/power", get(handle_power))
        .route_layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(10),
        ))
        .with_state(receiver);

    router.nest("/avreceiver", avreceiver_router)
}

async fn handle_volume(
    State(receiver): State<AVReceiver>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let mut query: std::collections::HashMap<std::borrow::Cow<str>, std::borrow::Cow<str>> =
        form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()).collect();

    let mute = query.remove("mute");
    let volume = query.remove("volume");

    if !query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "Accepted parameters are 'mute', 'volume'",
        )
            .into_response();
    }

    if let Some(mute) = mute {
        let mute = mute.to_lowercase();
        if mute != "true" && mute != "false" {
            return (
                StatusCode::BAD_REQUEST,
                "Accepted values for mute are 'true', 'false'",
            )
                .into_response();
        }
        receiver.set_mute(mute == "true").await;
    }

    if let Some(volume) = volume {
        let volume = volume.to_lowercase();
        if volume == "increment" || volume == "decrement" {
            receiver.increment_volume(volume == "increment").await;
        } else {
            let volume = match volume.parse::<i16>() {
                Ok(volume) => volume,
                Err(_) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        "Accepted values for volume are 0 - 100, 'increment', 'decrement'",
                    )
                        .into_response();
                }
            };
            receiver.set_volume(volume).await;
        }
    }

    let (volume, mute) = receiver.get_volume().await;
    Json(serde_json::json!({
        "data": {
            "volume": volume,
            "mute": mute
        }
    }))
    .into_response()
}

async fn handle_power(
    State(receiver): State<AVReceiver>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let mut query: std::collections::HashMap<std::borrow::Cow<str>, std::borrow::Cow<str>> =
        form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()).collect();
    let power = query.remove("power");

    if !query.is_empty() {
        return (StatusCode::BAD_REQUEST, "Accepted parameters are 'power'").into_response();
    }

    if let Some(power) = power {
        let power = power.to_lowercase();
        if power != "on" && power != "off" {
            return (
                StatusCode::BAD_REQUEST,
                "Accepted values for power are 'on', 'off'",
            )
                .into_response();
        }
        receiver.set_power(power == "on").await;
    }

    let power = receiver.is_powered_on().await;
    Json(serde_json::json!({
        "data": {
            "power": power
        }
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use tower::ServiceExt;

    #[tokio::test]
    async fn it_allows_setting_volume() {
        let mut receiver_mock = crate::avreceiver::MockAVReceiver::new();
        receiver_mock
            .expect_set_volume()
            .with(mockall::predicate::eq(25))
            .times(1)
            .returning(|_| 20);
        receiver_mock
            .expect_increment_volume()
            .with(mockall::predicate::eq(true))
            .times(1)
            .returning(|_| 20);
        receiver_mock
            .expect_get_volume()
            .times(2)
            .returning(|| (25, false));

        let router = super::add_routes(axum::Router::new(), std::sync::Arc::new(receiver_mock));
        for uri in [
            "/avreceiver/volume?volume=25",
            "/avreceiver/volume?volume=increment",
        ] {
            let request = hyper::Request::builder()
                .uri(uri)
                .body(axum::body::Body::empty())
                .unwrap();
            let response = router.clone().oneshot(request).await.unwrap();
            assert_eq!(200, response.status());
        }
    }

    #[tokio::test]
    async fn it_allows_powering() {
        let mut receiver_mock = crate::avreceiver::MockAVReceiver::new();
        receiver_mock
            .expect_set_power()
            .with(mockall::predicate::eq(true))
            .times(1)
            .returning(|_| true);
        receiver_mock
            .expect_is_powered_on()
            .times(1)
            .returning(|| true);

        let router = super::add_routes(axum::Router::new(), std::sync::Arc::new(receiver_mock));
        let request = hyper::Request::builder()
            .uri("/avreceiver/power?power=on")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(200, response.status());
    }

    #[tokio::test]
    async fn it_rejects_unknown_volume_parameters() {
        let router = super::add_routes(
            axum::Router::new(),
            std::sync::Arc::new(crate::avreceiver::MockAVReceiver::new()),
        );
        let request = hyper::Request::builder()
            .uri("/avreceiver/volume?unknown=value")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(400, response.status());
    }
}
