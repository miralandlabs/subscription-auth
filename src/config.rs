use crate::error::Error;

#[derive(Clone, Debug)]
pub struct Config {
    pub hmac_secret: Vec<u8>,
    pub rsa_private_key_pem: String,
    pub key_id: String,
    pub iss: String,
    pub revocation_window_days: i64,
}

impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let hmac_secret = std::env::var("SUBSCRIPTION_AUTH_HMAC_SECRET")
            .map_err(|_| Error::Internal("SUBSCRIPTION_AUTH_HMAC_SECRET not set".into()))?;
        // Check length on raw bytes (not trimmed) to ensure the full secret is adequate.
        // Trimming before checking length would allow short secrets padded with spaces to pass.
        if hmac_secret.len() < 32 {
            return Err(Error::Internal(
                "SUBSCRIPTION_AUTH_HMAC_SECRET must be at least 32 bytes".into(),
            ));
        }

        let rsa_private_key_pem = std::env::var("SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM")
            .map_err(|_| Error::Internal("SUBSCRIPTION_AUTH_RSA_PRIVATE_KEY_PEM not set".into()))?;
        let key_id = std::env::var("SUBSCRIPTION_AUTH_KEY_ID")
            .map_err(|_| Error::Internal("SUBSCRIPTION_AUTH_KEY_ID not set".into()))?;
        let iss = std::env::var("SUBSCRIPTION_AUTH_ISS")
            .map_err(|_| Error::Internal("SUBSCRIPTION_AUTH_ISS not set".into()))?;

        let revocation_window_days = std::env::var("SUBSCRIPTION_AUTH_REVOCATION_WINDOW_DAYS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(7);

        Ok(Self {
            hmac_secret: hmac_secret.into_bytes(),
            rsa_private_key_pem,
            key_id,
            iss,
            revocation_window_days,
        })
    }
}
