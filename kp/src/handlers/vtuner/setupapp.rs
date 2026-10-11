use axum::{
    Router,
    body::Bytes,
    extract::{OriginalUri, State},
    http::{HeaderMap, Method, Request, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tower::ServiceExt;

#[derive(Clone)]
struct SetupappState {
    routes: Router,
}

// TODO check if the router is actually useful to us aas it's ugly
// in fact, double check what we need in this module
pub(super) fn get_router(routes: Router) -> Router {
    let state = SetupappState { routes };
    Router::new()
        .route("/setupapp/", get(dispatch).post(dispatch))
        .route("/setupapp/{*path}", get(dispatch).post(dispatch))
        .with_state(state)
}

async fn dispatch(
    State(state): State<SetupappState>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if query_value(&uri, "token").as_deref() == Some("0") {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
            "<EncryptedToken>0000000000000000</EncryptedToken>",
        )
            .into_response();
    }

    let path = uri.path();
    let target = if query_value(&uri, "search").is_some() {
        "/search/"
    } else if path.contains("statxml.asp") && query_value(&uri, "id").is_some() {
        "/station"
    } else if path.contains("navXML.asp") {
        "/radiobrowser/"
    } else if path.contains("FavXML.asp") {
        "/my_stations/"
    } else if path.contains("loginXML.asp") {
        "/"
    } else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let target_uri = match uri.query() {
        Some(query) => format!("{target}?{query}").parse::<Uri>(),
        None => target.parse::<Uri>(),
    };
    let Ok(target_uri) = target_uri else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let mut request = Request::builder()
        .method(method)
        .uri(target_uri)
        .body(axum::body::Body::from(body))
        .expect("internal compatibility request is valid");
    *request.headers_mut() = headers;

    match state.routes.clone().oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    }
}

fn query_value(uri: &Uri, key: &str) -> Option<String> {
    form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::to_bytes,
        http::{Method, StatusCode},
        routing::any,
    };
    use tower::ServiceExt;

    use super::*;

    async fn marker() -> &'static str {
        "dispatched"
    }

    fn target_router() -> Router {
        Router::new()
            .route("/", any(marker))
            .route("/search/", any(marker))
            .route("/station", any(marker))
            .route("/radiobrowser/", any(marker))
            .route("/my_stations/", any(marker))
    }

    async fn dispatches(path: &str, method: Method) -> Response {
        get_router(target_router())
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn body(response: Response) -> String {
        String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn it_returns_the_init_token_for_get_and_post() {
        for method in [Method::GET, Method::POST] {
            let response = dispatches("/setupapp/?token=0", method).await;
            assert_eq!(StatusCode::OK, response.status());
            assert_eq!(
                "<EncryptedToken>0000000000000000</EncryptedToken>",
                body(response).await
            );
        }
    }

    #[tokio::test]
    async fn it_dispatches_compatibility_paths_for_get_and_post() {
        for method in [Method::GET, Method::POST] {
            for (path, target) in [
                ("/setupapp/?search=jazz", "/search/"),
                ("/setupapp/statxml.asp?id=RB_uuid", "/station"),
                ("/setupapp/navXML.asp", "/radiobrowser/"),
                ("/setupapp/FavXML.asp", "/my_stations/"),
                ("/setupapp/loginXML.asp", "/"),
            ] {
                let response = dispatches(path, method.clone()).await;
                assert_eq!(StatusCode::OK, response.status(), "{path}");
                assert_eq!("dispatched", body(response).await, "{target}");
            }
        }
    }

    #[tokio::test]
    async fn it_preserves_query_parameters_when_dispatching() {
        let routes = Router::new().route(
            "/station",
            any(|OriginalUri(uri): OriginalUri| async move {
                uri.query().unwrap_or_default().to_owned()
            }),
        );
        let response = get_router(routes)
            .oneshot(
                Request::builder()
                    .uri("/setupapp/statxml.asp?id=RB_uuid&extra=value")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!("id=RB_uuid&extra=value", body(response).await);
    }

    #[tokio::test]
    async fn it_returns_not_found_for_unhandled_setupapp_paths() {
        let response = dispatches("/setupapp/other.asp", Method::GET).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());
    }
}
