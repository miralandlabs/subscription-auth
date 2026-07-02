use {
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine},
    rsa::{
        pkcs1::DecodeRsaPrivateKey, pkcs8::DecodePrivateKey, traits::PublicKeyParts, RsaPrivateKey,
    },
    serde_json::{json, Value},
    tracing::error,
};

use crate::{config::Config, error::Error};

pub fn public_jwk_from_private_pem(pem: &str, kid: &str) -> Result<Value, Error> {
    let private_key = RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|e| Error::Internal(format!("invalid RSA private key: {e}")))?;
    let public_key = private_key.to_public_key();
    let n = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
    let e = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
    Ok(json!({
        "kty": "RSA",
        "kid": kid,
        "use": "sig",
        "alg": "RS256",
        "n": n,
        "e": e
    }))
}

pub fn build_jwks(current: &Config, db_keys: Vec<(String, Value)>) -> Value {
    let mut keys: Vec<Value> = Vec::new();
    match public_jwk_from_private_pem(&current.rsa_private_key_pem, &current.key_id) {
        Ok(jwk) => keys.push(jwk),
        Err(e) => {
            // Log at ERROR level: an empty keys array means all incoming JWTs will fail
            // signature verification until the operator fixes the key configuration.
            error!(error = %e, key_id = %current.key_id, "failed to build JWK from private key — JWKS will be empty");
        }
    }
    for (kid, jwk) in db_keys {
        if keys
            .iter()
            .any(|k| k.get("kid").and_then(|v| v.as_str()) == Some(kid.as_str()))
        {
            continue;
        }
        keys.push(jwk);
    }
    json!({ "keys": keys })
}
