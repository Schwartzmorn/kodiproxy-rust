use std::str::FromStr;

use axum::{
    Router,
    extract::{OriginalUri, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::timeout::TimeoutLayer;

use crate::cec::CECError;

type CecConnection = std::sync::Arc<std::sync::Mutex<dyn crate::cec::CECInterface>>;

pub fn add_routes(router: Router, connection: CecConnection) -> Router {
    let cec_router = Router::new()
        .route("/power-on", get(power_on))
        .route("/standby", get(standby))
        .route_layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(5),
        ))
        .with_state(connection);

    router.nest("/cec", cec_router)
}

fn parse_address(uri: &Uri) -> Result<crate::cec::CECLogicalAddress, CECError> {
    form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
        .find(|(param, _)| param == "device")
        .map(|(_, value)| crate::cec::CECLogicalAddress::from_str(&value))
        .unwrap_or(Ok(crate::cec::CECLogicalAddress::Broadcast))
}

async fn power_on(
    State(connection): State<CecConnection>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let address = match parse_address(&uri) {
        Ok(address) => address,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "Invalid device parameter").into_response();
        }
    };

    let mut connection = match connection.lock() {
        Ok(connection) => connection,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Failed to acquire lock on CEC connection",
            )
                .into_response();
        }
    };

    match connection.power_on(address) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to turn on device: {:?}", error),
        )
            .into_response(),
    }
}

async fn standby(
    State(connection): State<CecConnection>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let address = match parse_address(&uri) {
        Ok(address) => address,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "Invalid device parameter").into_response();
        }
    };

    let mut connection = match connection.lock() {
        Ok(connection) => connection,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Failed to acquire lock on CEC connection",
            )
                .into_response();
        }
    };

    match connection.standby(address) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed put device in standby: {:?}", error),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use tower::ServiceExt;

    #[tokio::test]
    async fn it_powers_on_the_requested_device() {
        let mut connection = crate::cec::MockCECInterface::new();
        connection
            .expect_power_on()
            .with(mockall::predicate::eq(crate::cec::CECLogicalAddress::TV))
            .times(1)
            .return_const(Ok(()));
        let connection: super::CecConnection =
            std::sync::Arc::new(std::sync::Mutex::new(connection));
        let router = super::add_routes(axum::Router::new(), connection);
        let request = hyper::Request::builder()
            .uri("/cec/power-on?device=TV")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(204, response.status());
    }

    #[tokio::test]
    async fn it_rejects_an_invalid_device() {
        let router = super::add_routes(
            axum::Router::new(),
            std::sync::Arc::new(std::sync::Mutex::new(crate::cec::MockCECInterface::new())),
        );
        let request = hyper::Request::builder()
            .uri("/cec/standby?device=invalid")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(400, response.status());
    }
}
