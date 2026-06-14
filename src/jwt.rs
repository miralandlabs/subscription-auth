use {
    chrono::{DateTime, Utc},
    jsonwebtoken::{encode, Algorithm, EncodingKey, Header},
    serde::{Deserialize, Serialize},
    serde_json::Value,
    uuid::Uuid,
};

use crate::{config::Config, error::Error, tiers};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub iss: String,
    pub sub: String,
    pub payer: String,
    pub tier: String,
    pub resources: Vec<String>,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

pub struct IssuedToken {
    pub token: String,
    pub jti: Uuid,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub resources: Value,
}

pub fn issue_rs256_token(
    config: &Config,
    service_id: &str,
    payer: &str,
    tier: &str,
    resources: Vec<String>,
) -> Result<IssuedToken, Error> {
    tiers::validate_tier(tier)?;

    let issued_at = Utc::now();
    let dur = tiers::tier_duration_secs(tier)
        .ok_or_else(|| Error::Internal(format!("unknown tier: {tier}")))?;
    let expires_at = issued_at + chrono::Duration::seconds(dur);
    let jti = Uuid::new_v4();

    let claims = TokenClaims {
        iss: config.iss.clone(),
        sub: service_id.to_string(),
        payer: payer.to_string(),
        tier: tier.to_string(),
        resources: resources.clone(),
        jti: jti.to_string(),
        iat: issued_at.timestamp(),
        exp: expires_at.timestamp(),
    };

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(config.key_id.clone());

    let encoding_key = EncodingKey::from_rsa_pem(config.rsa_private_key_pem.as_bytes())
        .map_err(|e| Error::Internal(format!("RSA encoding key: {e}")))?;

    let token = encode(&header, &claims, &encoding_key)
        .map_err(|e| Error::Internal(format!("jwt sign: {e}")))?;

    let resources_json =
        serde_json::to_value(&resources).map_err(|e| Error::Internal(format!("json: {e}")))?;

    Ok(IssuedToken {
        token,
        jti,
        issued_at,
        expires_at,
        resources: resources_json,
    })
}

pub fn decode_bearer_token(auth_header: Option<&str>) -> Result<String, Error> {
    let header =
        auth_header.ok_or_else(|| Error::Unauthorized("missing Authorization header".into()))?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| Error::Unauthorized("expected Bearer token".into()))?
        .trim();
    if token.is_empty() {
        return Err(Error::Unauthorized("empty Bearer token".into()));
    }
    Ok(token.to_string())
}

pub fn decode_unverified_claims(token: &str) -> Result<TokenClaims, Error> {
    use jsonwebtoken::{decode, DecodingKey, Validation};
    let mut validation = Validation::new(Algorithm::RS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;

    let data = decode::<TokenClaims>(token, &DecodingKey::from_secret(&[]), &validation)
        .map_err(|e| Error::Unauthorized(format!("invalid token: {e}")))?;
    Ok(data.claims)
}
