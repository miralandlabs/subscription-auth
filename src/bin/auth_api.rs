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

async fn body_to_string(body: Body) -> String {
    match body {
        Body::Text(s) => s,
        Body::Binary(b) => String::from_utf8(b).unwrap_or_default(),
        Body::Empty => String::new(),
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
            let body = body_to_string(req.into_body()).await;

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
                ("GET", p) if p.starts_with("/v1/services/") && p.ends_with("/subscriptions") => {
                    match api::parse_service_wallet(p, "/subscriptions") {
                        Some(wallet) => api::handle_list_subscriptions(state, wallet, &query).await,
                        None => not_found(),
                    }
                }
                ("POST", "/v1/tokens/issue") => api::handle_issue(state, body).await,
                ("POST", "/v1/tokens/revoke") => api::handle_revoke(state, body).await,
                ("POST", "/v1/tokens/introspect") => {
                    api::handle_introspect(state, auth.as_deref()).await
                }
                ("GET", "/v1/revocations") => api::handle_revocations(state, &query).await,
                _ => not_found(),
            };

            Ok(response)
        }
    })
    .await
}

fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::Text("Not found".into()))
        .unwrap()
}
