use vercel_runtime::{Body, Response, StatusCode as VercelStatusCode};

pub fn add_cors_headers(builder: http::response::Builder) -> http::response::Builder {
    builder
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization",
        )
}

pub fn json_response<T: serde::Serialize>(status: u16, value: &T) -> Response<Body> {
    let vercel_status = VercelStatusCode::from_u16(status).unwrap_or(VercelStatusCode::OK);
    let builder = Response::builder()
        .status(vercel_status)
        .header("Content-Type", "application/json");

    add_cors_headers(builder)
        .body(Body::Text(
            serde_json::to_string(value).unwrap_or_else(|_| "{}".into()),
        ))
        .unwrap()
}

pub fn cors_options() -> Response<Body> {
    let builder = Response::builder()
        .status(VercelStatusCode::NO_CONTENT)
        .header("Access-Control-Max-Age", "86400");

    add_cors_headers(builder).body(Body::Empty).unwrap()
}

pub fn parse_wallet_path(path: &str, suffix: &str) -> Option<String> {
    let prefix = "/v1/services/";
    let rest = path.strip_prefix(prefix)?;
    let wallet = rest.strip_suffix(suffix)?.trim_end_matches('/');
    if wallet.is_empty() {
        return None;
    }
    Some(wallet.to_string())
}

/// Standard `application/x-www-form-urlencoded` query string (`a=1&b=2`).
pub fn parse_query_map(query: &str) -> std::collections::HashMap<String, String> {
    if query.trim().is_empty() {
        return std::collections::HashMap::new();
    }
    serde_qs::from_str(query).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_query_map_standard() {
        let m = parse_query_map("action=register&service_id=e2e.local.ipay.sh");
        assert_eq!(m.get("action").map(String::as_str), Some("register"));
        assert_eq!(
            m.get("service_id").map(String::as_str),
            Some("e2e.local.ipay.sh")
        );
    }
}
