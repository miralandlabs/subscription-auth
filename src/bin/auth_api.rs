use {
    std::sync::Arc,
    subscription_auth::{api, http_util, state::AppState},
    tracing_subscriber::{fmt, EnvFilter},
    vercel_runtime::{run, Body, Request, Response, StatusCode},
};

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("subscription_auth=info,server_log=info"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stdout)
        .try_init();
}

async fn body_to_string(body: Body) -> Result<String, String> {
    const MAX_BODY_SIZE: usize = 1_048_576; // 1MB

    // Check size first to avoid duplication
    let len = match &body {
        Body::Text(s) => s.len(),
        Body::Binary(b) => b.len(),
        Body::Empty => 0,
    };

    if len > MAX_BODY_SIZE {
        return Err(format!(
            "request body too large: {} bytes (max {})",
            len, MAX_BODY_SIZE
        ));
    }

    // Convert body after size check passes
    match body {
        Body::Text(s) => Ok(s),
        Body::Binary(b) => {
            String::from_utf8(b).map_err(|_| "request body is not valid UTF-8".to_string())
        }
        Body::Empty => Ok(String::new()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_tracing();
    let state = Arc::new(AppState::new()?);

    run(move |req: Request| {
        let state = Arc::clone(&state);
        async move {
            let method = req.method().clone();
            let path = req.uri().path().to_string();
            let query = req.uri().query().unwrap_or("").to_string();
            let auth = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            let body = match body_to_string(req.into_body()).await {
                Ok(s) => s,
                Err(e) => {
                    return Ok(http_util::json_response(
                        400,
                        &serde_json::json!({ "error": "BAD_REQUEST", "message": e }),
                    ));
                }
            };

            if method == http::Method::OPTIONS {
                return Ok(api::route_options());
            }

            let response = match (method.as_str(), path.as_str()) {
                ("GET", "/health") => api::handle_health(state).await,
                ("GET", "/.well-known/jwks.json") => api::handle_jwks(state).await,
                ("GET", "/openapi.json") => http_util::json_response(
                    200,
                    &serde_json::json!({ "note": "See public/openapi.json in repo" }),
                ),
                ("GET", p) if p.starts_with("/v1/services/") && p.ends_with("/challenge") => {
                    match api::parse_service_wallet(p, "/challenge") {
                        Some(wallet) => api::handle_challenge(state, wallet, &query).await,
                        None => not_found(),
                    }
                }
                ("POST", p) if p.starts_with("/v1/services/") && p.ends_with("/register") => {
                    match api::parse_service_wallet(p, "/register") {
                        Some(wallet) => api::handle_register(state, wallet, body).await,
                        None => not_found(),
                    }
                }
                ("POST", p) if p.starts_with("/v1/services/") && p.ends_with("/update") => {
                    match api::parse_service_wallet(p, "/update") {
                        Some(wallet) => api::handle_update(state, wallet, body).await,
                        None => not_found(),
                    }
                }
                ("POST", p) if p.starts_with("/v1/services/") && p.ends_with("/retire") => {
                    match api::parse_service_wallet(p, "/retire") {
                        Some(wallet) => api::handle_retire(state, wallet, body).await,
                        None => not_found(),
                    }
                }
                // Subscriptions: POST so auth data (message+sig) stays in the body,
                // not query params where it would appear in server logs (BUG-04/UX-05).
                ("POST", p) if p.starts_with("/v1/services/") && p.ends_with("/subscriptions") => {
                    match api::parse_service_wallet(p, "/subscriptions") {
                        Some(wallet) => api::handle_list_subscriptions(state, wallet, body).await,
                        None => not_found(),
                    }
                }
                ("POST", "/v1/tokens/issue") => api::handle_issue(state, body).await,
                ("POST", "/v1/tokens/revoke") => api::handle_revoke(state, body).await,
                ("POST", "/v1/tokens/introspect") => {
                    api::handle_introspect(state, auth.as_deref()).await
                }
                ("GET", "/v1/revocations") => api::handle_revocations(state, &query).await,
                // Unauthenticated service info — lets sellers check registration without re-registering.
                ("GET", p) if p.starts_with("/v1/info/") => {
                    let service_id = p.trim_start_matches("/v1/info/").to_string();
                    if service_id.is_empty() {
                        not_found()
                    } else {
                        api::handle_service_info(state, service_id).await
                    }
                }
                _ => not_found(),
            };

            Ok(response)
        }
    })
    .await
}

fn not_found() -> Response<Body> {
    let builder = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/plain");
    http_util::add_cors_headers(builder)
        .body(Body::Text("Not found".into()))
        .unwrap()
}
