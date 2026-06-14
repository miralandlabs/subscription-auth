use vercel_runtime::{Body, Response, StatusCode as VercelStatusCode};

pub fn json_response<T: serde::Serialize>(status: u16, value: &T) -> Response<Body> {
    let vercel_status = VercelStatusCode::from_u16(status).unwrap_or(VercelStatusCode::OK);
    Response::builder()
        .status(vercel_status)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization",
        )
        .body(Body::Text(
            serde_json::to_string(value).unwrap_or_else(|_| "{}".into()),
        ))
        .unwrap()
}

pub fn cors_options() -> Response<Body> {
    Response::builder()
        .status(VercelStatusCode::NO_CONTENT)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization",
        )
        .header("Access-Control-Max-Age", "86400")
        .body(Body::Empty)
        .unwrap()
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
