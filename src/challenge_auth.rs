//! Wallet-signed admin writes: HMAC-bound challenge + ed25519 signature.
//! Domain: `x402 subscription auth v1` (≠ pr402 onboard).

use {
    hmac::{Hmac, Mac},
    sha2::Sha256,
    solana_pubkey::Pubkey,
    solana_signature::Signature,
    std::str::FromStr,
    std::time::{SystemTime, UNIX_EPOCH},
    subtle::ConstantTimeEq,
};

type HmacSha256 = Hmac<Sha256>;

pub const DOMAIN: &str = "x402 subscription auth v1";
const HMAC_LINE_PREFIX: &str = "hmac_sha256_hex: ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Register,
    Issue,
    Revoke,
    Update,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Register => "register",
            Action::Issue => "issue",
            Action::Revoke => "revoke",
            Action::Update => "update",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "register" => Ok(Action::Register),
            "issue" => Ok(Action::Issue),
            "revoke" => Ok(Action::Revoke),
            "update" => Ok(Action::Update),
            _ => Err(format!("unknown action: {s}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChallengeBuildParams {
    pub action: Action,
    pub service_id: Option<String>,
    pub service_url: Option<String>,
    pub resources_allowlist_json: Option<String>,
    pub payer: Option<String>,
    pub tier: Option<String>,
    pub resources_json: Option<String>,
    pub jti: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedChallenge {
    pub action: Action,
    pub wallet: String,
    pub nonce: String,
    pub issued_unix: u64,
    pub expires_unix: u64,
    pub service_id: Option<String>,
    pub service_url: Option<String>,
    pub resources_allowlist_json: Option<String>,
    pub payer: Option<String>,
    pub tier: Option<String>,
    pub resources_json: Option<String>,
    pub jti: Option<String>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").ok();
    }
    s
}

fn compute_hmac_hex(secret: &[u8], preimage: &str) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| "invalid HMAC key length")?;
    mac.update(preimage.as_bytes());
    Ok(hex_lower(&mac.finalize().into_bytes()))
}

fn random_nonce_hex() -> Result<String, String> {
    let mut nonce = [0u8; 16];
    getrandom::getrandom(&mut nonce).map_err(|_| "RNG failure")?;
    Ok(hex_lower(&nonce))
}

fn decode_signature_flexible(encoded: &str) -> Result<Signature, String> {
    let raw = encoded.trim();
    if raw.is_empty() {
        return Err("empty signature".into());
    }
    if let Ok(sig) = Signature::from_str(raw) {
        return Ok(sig);
    }
    use base64::{engine::general_purpose::STANDARD as B64_STD, Engine};
    let bytes = B64_STD
        .decode(raw)
        .map_err(|_| "invalid signature encoding (expected base58 or base64)")?;
    if bytes.len() != 64 {
        return Err(format!(
            "invalid signature length: expected 64 bytes, got {}",
            bytes.len()
        ));
    }
    let arr: [u8; 64] = bytes
        .try_into()
        .map_err(|_| "invalid signature length after base64 decode")?;
    Ok(Signature::from(arr))
}

fn build_preimage(
    wallet: &str,
    issued: u64,
    expires: u64,
    nonce: &str,
    params: &ChallengeBuildParams,
) -> String {
    let mut lines = vec![
        DOMAIN.to_string(),
        format!("wallet: {wallet}"),
        format!("issued_unix: {issued}"),
        format!("expires_unix: {expires}"),
        format!("nonce: {nonce}"),
        format!("action: {}", params.action.as_str()),
    ];

    match params.action {
        Action::Register => {
            if let Some(ref sid) = params.service_id {
                lines.push(format!("service_id: {sid}"));
            }
            if let Some(ref url) = params.service_url {
                lines.push(format!("service_url: {url}"));
            }
            if let Some(ref json) = params.resources_allowlist_json {
                lines.push(format!("resources_allowlist_json: {json}"));
            }
        }
        Action::Issue => {
            if let Some(ref sid) = params.service_id {
                lines.push(format!("service_id: {sid}"));
            }
            if let Some(ref payer) = params.payer {
                lines.push(format!("payer: {payer}"));
            }
            if let Some(ref tier) = params.tier {
                lines.push(format!("tier: {tier}"));
            }
            if let Some(ref json) = params.resources_json {
                lines.push(format!("resources_json: {json}"));
            }
        }
        Action::Revoke => {
            if let Some(ref sid) = params.service_id {
                lines.push(format!("service_id: {sid}"));
            }
            if let Some(ref jti) = params.jti {
                lines.push(format!("jti: {jti}"));
            }
        }
        Action::Update => {
            if let Some(ref sid) = params.service_id {
                lines.push(format!("service_id: {sid}"));
            }
            if let Some(ref json) = params.resources_allowlist_json {
                lines.push(format!("resources_allowlist_json: {json}"));
            }
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

pub fn build_challenge_message(
    hmac_secret: &[u8],
    wallet_b58: &str,
    ttl_sec: u64,
    params: ChallengeBuildParams,
) -> Result<(String, u64), String> {
    if ttl_sec == 0 || ttl_sec > 3600 {
        return Err("ttl_sec must be 1..=3600".into());
    }
    Pubkey::from_str(wallet_b58).map_err(|_| "invalid wallet pubkey")?;

    let nonce = random_nonce_hex()?;
    let issued = now_unix();
    let expires = issued.saturating_add(ttl_sec);
    let preimage = build_preimage(wallet_b58, issued, expires, &nonce, &params);
    let hmac_hex = compute_hmac_hex(hmac_secret, &preimage)?;
    let message = format!("{preimage}{HMAC_LINE_PREFIX}{hmac_hex}");
    Ok((message, expires))
}

fn parse_line_value<'a>(line: &'a str, prefix: &str) -> Result<&'a str, String> {
    line.strip_prefix(prefix)
        .ok_or_else(|| format!("expected line prefix: {prefix}"))
}

pub fn verify_challenge_submission(
    hmac_secret: &[u8],
    wallet_b58: &str,
    message: &str,
    signature_encoded: &str,
) -> Result<ParsedChallenge, String> {
    let pk = Pubkey::from_str(wallet_b58).map_err(|_| "invalid wallet pubkey")?;
    let sig = decode_signature_flexible(signature_encoded)?;

    let Some(idx) = message.rfind(HMAC_LINE_PREFIX) else {
        return Err("missing HMAC line".into());
    };
    let body = &message[..idx];
    let hmac_line = &message[idx..];
    let hmac_hex = hmac_line
        .strip_prefix(HMAC_LINE_PREFIX)
        .ok_or_else(|| "invalid HMAC line".to_string())?;
    if hmac_hex.len() != 64 || !hmac_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid HMAC hex".into());
    }

    let expected_mac = compute_hmac_hex(hmac_secret, body)?;
    if expected_mac.len() != hmac_hex.len()
        || !bool::from(expected_mac.as_bytes().ct_eq(hmac_hex.as_bytes()))
    {
        return Err("HMAC mismatch".into());
    }

    let mut lines: Vec<&str> = body.lines().collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    if lines.len() < 6 {
        return Err("unexpected message line count".into());
    }
    if lines[0] != DOMAIN {
        return Err("invalid domain".into());
    }
    let wallet_line = parse_line_value(lines[1], "wallet: ")?;
    if wallet_line != wallet_b58 {
        return Err("wallet mismatch".into());
    }
    let issued = parse_line_value(lines[2], "issued_unix: ")?
        .parse::<u64>()
        .map_err(|_| "issued_unix".to_string())?;
    let expires = parse_line_value(lines[3], "expires_unix: ")?
        .parse::<u64>()
        .map_err(|_| "expires_unix".to_string())?;
    let nonce = parse_line_value(lines[4], "nonce: ")?;
    if nonce.len() != 32 || !nonce.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid nonce".into());
    }
    let action_str = parse_line_value(lines[5], "action: ")?;
    let action = Action::parse(action_str)?;

    let t = now_unix();
    if t < issued {
        return Err("issued in the future".into());
    }
    if t > expires {
        return Err("challenge expired".into());
    }

    if !sig.verify(pk.as_ref(), message.as_bytes()) {
        return Err("invalid signature".into());
    }

    let mut parsed = ParsedChallenge {
        action: action.clone(),
        wallet: wallet_b58.to_string(),
        nonce: nonce.to_string(),
        issued_unix: issued,
        expires_unix: expires,
        service_id: None,
        service_url: None,
        resources_allowlist_json: None,
        payer: None,
        tier: None,
        resources_json: None,
        jti: None,
    };

    for line in &lines[6..] {
        if let Some(v) = line.strip_prefix("service_id: ") {
            parsed.service_id = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("service_url: ") {
            parsed.service_url = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("resources_allowlist_json: ") {
            parsed.resources_allowlist_json = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("payer: ") {
            parsed.payer = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("tier: ") {
            parsed.tier = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("resources_json: ") {
            parsed.resources_json = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("jti: ") {
            parsed.jti = Some(v.to_string());
        }
    }

    Ok(parsed)
}

pub fn minify_json_array(values: &[String]) -> Result<String, String> {
    serde_json::to_string(values).map_err(|e| format!("json encode: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_signer::Signer;

    fn secret() -> Vec<u8> {
        b"test-secret-at-least-32-bytes-long!!".to_vec()
    }

    #[test]
    fn register_round_trip() {
        let kp = solana_keypair::Keypair::new();
        let wallet = kp.pubkey().to_string();
        let allowlist = minify_json_array(&["/api/v1/echo".to_string()]).unwrap();
        let (msg, _exp) = build_challenge_message(
            &secret(),
            &wallet,
            600,
            ChallengeBuildParams {
                action: Action::Register,
                service_id: Some("fifa.example.com".into()),
                service_url: Some("https://fifa.example.com".into()),
                resources_allowlist_json: Some(allowlist),
                payer: None,
                tier: None,
                resources_json: None,
                jti: None,
            },
        )
        .unwrap();
        let sig = kp.sign_message(msg.as_bytes());
        let parsed = verify_challenge_submission(&secret(), &wallet, &msg, &sig.to_string())
            .unwrap();
        assert_eq!(parsed.action, Action::Register);
        assert_eq!(parsed.service_id.as_deref(), Some("fifa.example.com"));
    }

    #[test]
    fn issue_round_trip() {
        let kp = solana_keypair::Keypair::new();
        let wallet = kp.pubkey().to_string();
        let resources = minify_json_array(&["*".to_string()]).unwrap();
        let (msg, _exp) = build_challenge_message(
            &secret(),
            &wallet,
            600,
            ChallengeBuildParams {
                action: Action::Issue,
                service_id: Some("fifa.example.com".into()),
                service_url: None,
                resources_allowlist_json: None,
                payer: Some("Buyer1111111111111111111111111111111111111".into()),
                tier: Some("hourly".into()),
                resources_json: Some(resources),
                jti: None,
            },
        )
        .unwrap();
        let sig = kp.sign_message(msg.as_bytes());
        let parsed = verify_challenge_submission(&secret(), &wallet, &msg, &sig.to_string())
            .unwrap();
        assert_eq!(parsed.action, Action::Issue);
        assert_eq!(parsed.tier.as_deref(), Some("hourly"));
    }

    #[test]
    fn hmac_tamper_fails() {
        let kp = solana_keypair::Keypair::new();
        let wallet = kp.pubkey().to_string();
        let (mut msg, _exp) = build_challenge_message(
            &secret(),
            &wallet,
            600,
            ChallengeBuildParams {
                action: Action::Revoke,
                service_id: Some("fifa.example.com".into()),
                service_url: None,
                resources_allowlist_json: None,
                payer: None,
                tier: None,
                resources_json: None,
                jti: Some("550e8400-e29b-41d4-a716-446655440000".into()),
            },
        )
        .unwrap();
        let sig = kp.sign_message(msg.as_bytes());
        msg.push('x');
        assert!(verify_challenge_submission(&secret(), &wallet, &msg, &sig.to_string()).is_err());
    }
}
