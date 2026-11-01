use crate::routes_table;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

#[derive(Clone)]
pub struct ProxyState {
    pub http: reqwest::Client,
}

/// Catch-all proxy handler. Strips nothing from the path (the full
/// original path is forwarded, including the matched prefix) since
/// every downstream service's own router expects its routes rooted at
/// that same prefix (e.g. train-service listens on `/trains/...`, not
/// bare `/...`) — the gateway and each service's route table are kept
/// in sync by convention, documented in `routes_table.rs`.
pub async fn proxy(State(state): State<ProxyState>, req: Request) -> Response {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(req.uri().path())
        .to_string();

    let Some((base_url, _matched_prefix)) = routes_table::resolve(req.uri().path()) else {
        return (StatusCode::NOT_FOUND, "no route configured for this path").into_response();
    };

    let target_url = format!("{base_url}{path_and_query}");
    if reqwest::Url::parse(&target_url).is_err() {
        return (StatusCode::BAD_GATEWAY, "failed to construct upstream URL").into_response();
    }

    let method = reqwest_method_from_axum(req.method());
    let headers = req.headers().clone();

    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "request body too large or unreadable",
            )
                .into_response()
        }
    };

    let mut upstream_req = state
        .http
        .request(method, &target_url)
        .body(body_bytes.to_vec());
    upstream_req = forward_headers(upstream_req, &headers);

    let upstream_response = match upstream_req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, target = %target_url, "upstream request failed");
            return (StatusCode::BAD_GATEWAY, "upstream service unreachable").into_response();
        }
    };

    let status = upstream_response.status();
    let response_headers = upstream_response.headers().clone();
    let response_body = match upstream_response.bytes().await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_GATEWAY,
                "failed to read upstream response body",
            )
                .into_response()
        }
    };

    let mut response = Response::builder()
        .status(status.as_u16())
        .body(Body::from(response_body))
        .unwrap();
    let out_headers = response.headers_mut();
    for (name, value) in response_headers.iter() {
        // hop-by-hop headers are not forwarded; everything else
        // (content-type, cache-control, custom headers) passes through.
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            axum::http::HeaderName::from_bytes(name.as_str().as_bytes()),
            axum::http::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            out_headers.insert(name, value);
        }
    }

    response
}

fn reqwest_method_from_axum(method: &axum::http::Method) -> reqwest::Method {
    reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET)
}

fn forward_headers(
    mut builder: reqwest::RequestBuilder,
    headers: &HeaderMap,
) -> reqwest::RequestBuilder {
    for (name, value) in headers.iter() {
        if is_hop_by_hop(name.as_str()) || name.as_str().eq_ignore_ascii_case("host") {
            continue;
        }
        if let (Ok(name), Ok(value_str)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            value.to_str(),
        ) {
            builder = builder.header(name, value_str);
        }
    }
    builder
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}
