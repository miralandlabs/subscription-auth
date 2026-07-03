use {
    chrono::Utc,
    serde::Serialize,
    serde_json::json,
    vercel_runtime::{Body, Response, StatusCode as VercelStatusCode},
};

#[derive(Debug)]
pub enum Error {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    /// 409 Conflict — used for idempotent operations where the resource already exists.
    Conflict(String),
    ServiceUnavailable(String),
    Internal(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BadRequest(msg) => write!(f, "Bad Request: {msg}"),
            Error::Unauthorized(msg) => write!(f, "Unauthorized: {msg}"),
            Error::Forbidden(msg) => write!(f, "Forbidden: {msg}"),
            Error::NotFound(msg) => write!(f, "Not Found: {msg}"),
            Error::Conflict(msg) => write!(f, "Conflict: {msg}"),
            Error::ServiceUnavailable(msg) => write!(f, "Service Unavailable: {msg}"),
            Error::Internal(msg) => write!(f, "Internal Error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    pub fn to_vercel_response(&self) -> Response<Body> {
        let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let (status, code, message) = match self {
            Error::BadRequest(msg) => (VercelStatusCode::BAD_REQUEST, "BAD_REQUEST", msg),
            Error::Unauthorized(msg) => (VercelStatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg),
            Error::Forbidden(msg) => (VercelStatusCode::FORBIDDEN, "FORBIDDEN", msg),
            Error::NotFound(msg) => (VercelStatusCode::NOT_FOUND, "NOT_FOUND", msg),
            Error::Conflict(msg) => (VercelStatusCode::CONFLICT, "CONFLICT", msg),
            Error::ServiceUnavailable(msg) => (
                VercelStatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                msg,
            ),
            Error::Internal(msg) => (
                VercelStatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                msg,
            ),
        };

        let builder = Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .header("X-Date", date);

        crate::http_util::add_cors_headers(builder)
            .body(Body::Text(
                json!({ "error": code, "message": message }).to_string(),
            ))
            .unwrap()
    }
}

pub fn into_vercel_response<T: Serialize>(result: Result<T, Error>) -> Response<Body> {
    match result {
        Ok(value) => crate::http_util::json_response(200, &value),
        Err(error) => error.to_vercel_response(),
    }
}
