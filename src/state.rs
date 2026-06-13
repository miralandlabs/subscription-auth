use {
    crate::{
        config::Config,
        db::AuthDb,
        error::Error,
        jwks,
    },
    std::sync::Arc,
};

pub struct AppState {
    pub config: Arc<Config>,
    pub db: Option<Arc<AuthDb>>,
}

impl AppState {
    pub fn new() -> Result<Self, Error> {
        let config = Arc::new(Config::from_env()?);
        let db = match AuthDb::from_env_var("DATABASE_URL") {
            None => None,
            Some(Ok(d)) => Some(Arc::new(d)),
            Some(Err(e)) => {
                tracing::warn!(error = %e, "DATABASE_URL set but connection failed");
                None
            }
        };
        Ok(Self { config, db })
    }

    pub fn require_db(&self) -> Result<Arc<AuthDb>, Error> {
        self.db
            .clone()
            .ok_or_else(|| Error::ServiceUnavailable("DATABASE_URL not configured".into()))
    }

    pub async fn jwks_document(&self) -> Result<serde_json::Value, Error> {
        let mut db_keys = Vec::new();
        if let Some(db) = &self.db {
            db_keys = db.list_signing_keys().await.unwrap_or_default();
        }
        Ok(jwks::build_jwks(&self.config, db_keys))
    }
}
