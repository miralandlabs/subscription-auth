mod resources;

use {
    chrono::{DateTime, Utc},
    serde::Deserialize,
    serde_json::{json, Value},
    std::sync::Arc,
    uuid::Uuid,
    vercel_runtime::{Body, Response},
};

use crate::{
    challenge_auth::{self, minify_json_array, Action, ChallengeBuildParams, ParsedChallenge},
    db::ServiceRow,
    error::{into_vercel_response, Error},
    http_util::{cors_options, json_response, parse_wallet_path},
    jwt::{self, decode_bearer_token, decode_unverified_claims},
    service_id::validate_service_id,
    state::AppState,
    tiers::{self, tier_label},
};

pub use resources::resources_subset_of_allowlist;

const CHALLENGE_TTL_SEC: u64 = 600;

#[derive(Deserialize)]
pub struct SignedBody {
    pub message: String,
    pub signature: String,
}

#[derive(Deserialize)]
pub struct RegisterBody {
    #[serde(flatten)]
    pub signed: SignedBody,
    pub service_id: String,
    pub service_url: String,
    pub resources_allowlist: Vec<String>,
    pub tier_bundles: Option<Value>,
    pub recovery_wallet: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateBody {
    #[serde(flatten)]
    pub signed: SignedBody,
    pub service_id: String,
    pub resources_allowlist: Vec<String>,
    pub tier_bundles: Option<Value>,
}

#[derive(Deserialize)]
pub struct IssueBody {
    #[serde(flatten)]
    pub signed: SignedBody,
}

#[derive(Deserialize)]
pub struct RevokeBody {
    #[serde(flatten)]
    pub signed: SignedBody,
}

async fn verify_and_consume(
    state: &AppState,
    wallet: &str,
    message: &str,
    signature: &str,
    expected_action: Action,
) -> Result<ParsedChallenge, Error> {
    let parsed = challenge_auth::verify_challenge_submission(
        &state.config.hmac_secret,
        wallet,
        message,
        signature,
    )
    .map_err(Error::Unauthorized)?;

    if parsed.action != expected_action {
        return Err(Error::Unauthorized("action mismatch".into()));
    }

    let db = state.require_db()?;
    let consumed = db
        .consume_nonce(wallet, &parsed.nonce)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    if !consumed {
        return Err(Error::Unauthorized(
            "nonce replay or expired challenge".into(),
        ));
    }
    Ok(parsed)
}

fn wallet_matches_path(wallet_path: &str, parsed_wallet: &str) -> Result<(), Error> {
    if wallet_path != parsed_wallet {
        return Err(Error::Unauthorized("wallet path mismatch".into()));
    }
    Ok(())
}

async fn load_active_service(state: &AppState, service_id: &str) -> Result<ServiceRow, Error> {
    let db = state.require_db()?;
    let row = db
        .get_service(service_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or_else(|| Error::NotFound("service not registered".into()))?;
    if row.status != "active" {
        return Err(Error::Forbidden("service retired".into()));
    }
    Ok(row)
}

pub async fn handle_health(state: Arc<AppState>) -> Response<Body> {
    let mut body = json!({
        "status": "ok",
        "service": "subscription-auth",
        "db": if state.db.is_some() { "configured" } else { "missing" }
    });
    if let Some(db) = &state.db {
        match db.ping().await {
            Ok(()) => body["db_ping"] = json!("ok"),
            Err(e) => body["db_ping"] = json!(format!("error: {e}")),
        }
    }
    json_response(200, &body)
}

pub async fn handle_jwks(state: Arc<AppState>) -> Response<Body> {
    match state.jwks_document().await {
        Ok(doc) => json_response(200, &doc),
        Err(e) => e.to_vercel_response(),
    }
}

pub async fn handle_challenge(state: Arc<AppState>, wallet: String, query: &str) -> Response<Body> {
    let result = async {
        let map = crate::http_util::parse_query_map(query);
        let action_str = map
            .get("action")
            .ok_or_else(|| Error::BadRequest("action query param required".into()))?;
        let action = Action::parse(action_str).map_err(Error::BadRequest)?;

        let build = match action {
            Action::Register => ChallengeBuildParams {
                action: Action::Register,
                service_id: map.get("service_id").cloned(),
                service_url: map.get("service_url").cloned(),
                resources_allowlist_json: map
                    .get("resources_allowlist_json")
                    .or_else(|| map.get("resources_allowlist"))
                    .cloned(),
                payer: None,
                tier: None,
                resources_json: None,
                jti: None,
            },
            Action::Issue => {
                let resources = map
                    .get("resources")
                    .or_else(|| map.get("resources_json"))
                    .cloned()
                    .unwrap_or_else(|| "[\"*\"]".into());
                ChallengeBuildParams {
                    action: Action::Issue,
                    service_id: map.get("service_id").cloned(),
                    service_url: None,
                    resources_allowlist_json: None,
                    payer: map.get("payer").cloned(),
                    tier: map.get("tier").cloned(),
                    resources_json: Some(resources),
                    jti: None,
                }
            }
            Action::Revoke => ChallengeBuildParams {
                action: Action::Revoke,
                service_id: map.get("service_id").cloned(),
                service_url: None,
                resources_allowlist_json: None,
                payer: None,
                tier: None,
                resources_json: None,
                jti: map.get("jti").cloned(),
            },
            Action::Update => ChallengeBuildParams {
                action: Action::Update,
                service_id: map.get("service_id").cloned(),
                service_url: None,
                resources_allowlist_json: map
                    .get("resources_allowlist_json")
                    .or_else(|| map.get("resources_allowlist"))
                    .cloned(),
                payer: None,
                tier: None,
                resources_json: None,
                jti: None,
            },
        };

        let (message, expires_unix) = challenge_auth::build_challenge_message(
            &state.config.hmac_secret,
            &wallet,
            CHALLENGE_TTL_SEC,
            build,
        )
        .map_err(Error::BadRequest)?;

        let nonce = message
            .lines()
            .find_map(|l| l.strip_prefix("nonce: "))
            .ok_or_else(|| Error::Internal("nonce missing".into()))?;

        let expires = DateTime::from_timestamp(expires_unix as i64, 0)
            .ok_or_else(|| Error::Internal("invalid expiry".into()))?;
        state
            .require_db()?
            .insert_nonce(&wallet, nonce, expires)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(json!({ "message": message, "expires_unix": expires_unix }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_register(
    state: Arc<AppState>,
    wallet: String,
    body_text: String,
) -> Response<Body> {
    let result = async {
        let body: RegisterBody = serde_json::from_str(&body_text)
            .map_err(|e| Error::BadRequest(format!("invalid json: {e}")))?;
        validate_service_id(&body.service_id)?;
        let parsed = verify_and_consume(
            &state,
            &wallet,
            &body.signed.message,
            &body.signed.signature,
            Action::Register,
        )
        .await?;
        wallet_matches_path(&wallet, &parsed.wallet)?;

        if parsed.service_id.as_deref() != Some(body.service_id.as_str()) {
            return Err(Error::Unauthorized(
                "service_id not bound in challenge".into(),
            ));
        }

        let allowlist_json =
            minify_json_array(&body.resources_allowlist).map_err(Error::BadRequest)?;
        if parsed.resources_allowlist_json.as_deref() != Some(allowlist_json.as_str()) {
            return Err(Error::Unauthorized(
                "resources_allowlist not bound in challenge".into(),
            ));
        }

        let db = state.require_db()?;
        if db
            .get_service(&body.service_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .is_some()
        {
            return Err(Error::Forbidden("service_id already registered".into()));
        }

        let allowlist_value: Value = serde_json::from_str(&allowlist_json)
            .map_err(|e| Error::BadRequest(format!("allowlist json: {e}")))?;

        db.insert_service(
            &body.service_id,
            &wallet,
            &body.service_url,
            &allowlist_value,
            body.tier_bundles.as_ref(),
        )
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(json!({
            "success": true,
            "service_id": body.service_id,
            "merchant_wallet": wallet,
            "service_url": body.service_url,
        }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_update(
    state: Arc<AppState>,
    wallet: String,
    body_text: String,
) -> Response<Body> {
    let result = async {
        let body: UpdateBody = serde_json::from_str(&body_text)
            .map_err(|e| Error::BadRequest(format!("invalid json: {e}")))?;
        let parsed = verify_and_consume(
            &state,
            &wallet,
            &body.signed.message,
            &body.signed.signature,
            Action::Update,
        )
        .await?;
        wallet_matches_path(&wallet, &parsed.wallet)?;

        let allowlist_json =
            minify_json_array(&body.resources_allowlist).map_err(Error::BadRequest)?;
        if parsed.service_id.as_deref() != Some(body.service_id.as_str()) {
            return Err(Error::Unauthorized("service_id mismatch".into()));
        }

        let service = load_active_service(&state, &body.service_id).await?;
        if service.merchant_wallet != wallet {
            return Err(Error::Unauthorized("not service merchant".into()));
        }

        let allowlist_value: Value = serde_json::from_str(&allowlist_json)
            .map_err(|e| Error::BadRequest(format!("allowlist json: {e}")))?;

        let updated = state
            .require_db()?
            .update_service(
                &body.service_id,
                &wallet,
                &allowlist_value,
                body.tier_bundles.as_ref(),
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        if !updated {
            return Err(Error::NotFound("service not found or not active".into()));
        }

        Ok(json!({ "success": true, "service_id": body.service_id }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_retire(
    state: Arc<AppState>,
    wallet: String,
    body_text: String,
) -> Response<Body> {
    let result = async {
        let body: SignedBody = serde_json::from_str(&body_text)
            .map_err(|e| Error::BadRequest(format!("invalid json: {e}")))?;
        let parsed = verify_and_consume(
            &state,
            &wallet,
            &body.message,
            &body.signature,
            Action::Update,
        )
        .await?;
        wallet_matches_path(&wallet, &parsed.wallet)?;

        let service_id = parsed
            .service_id
            .ok_or_else(|| Error::BadRequest("service_id required in challenge".into()))?;

        let retired = state
            .require_db()?
            .retire_service(&service_id, &wallet)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        if !retired {
            return Err(Error::NotFound("service not found".into()));
        }

        Ok(json!({ "success": true, "service_id": service_id, "status": "retired" }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_issue(state: Arc<AppState>, body_text: String) -> Response<Body> {
    let result = async {
        let body: IssueBody = serde_json::from_str(&body_text)
            .map_err(|e| Error::BadRequest(format!("invalid json: {e}")))?;

        // Wallet is in signed message — extract from parsed challenge
        let lines: Vec<&str> = body.signed.message.lines().collect();
        let wallet_line = lines
            .iter()
            .find_map(|l| l.strip_prefix("wallet: "))
            .ok_or_else(|| Error::BadRequest("wallet missing in message".into()))?;

        let parsed = verify_and_consume(
            &state,
            wallet_line,
            &body.signed.message,
            &body.signed.signature,
            Action::Issue,
        )
        .await?;

        let service_id = parsed
            .service_id
            .ok_or_else(|| Error::BadRequest("service_id required".into()))?;
        let payer = parsed
            .payer
            .ok_or_else(|| Error::BadRequest("payer required".into()))?;
        let tier = parsed
            .tier
            .ok_or_else(|| Error::BadRequest("tier required".into()))?;
        tiers::validate_tier(&tier)?;

        let resources_json = parsed
            .resources_json
            .ok_or_else(|| Error::BadRequest("resources_json required".into()))?;
        let resources: Vec<String> = serde_json::from_str(&resources_json)
            .map_err(|e| Error::BadRequest(format!("resources_json: {e}")))?;

        let service = load_active_service(&state, &service_id).await?;
        if service.merchant_wallet != wallet_line {
            return Err(Error::Unauthorized("not service merchant".into()));
        }

        if !resources_subset_of_allowlist(&resources, &service.resources_allowlist) {
            return Err(Error::Forbidden("resources not subset of allowlist".into()));
        }

        let issued =
            jwt::issue_rs256_token(&state.config, &service_id, &payer, &tier, resources.clone())?;

        state
            .require_db()?
            .insert_token(
                issued.jti,
                &service_id,
                &payer,
                &tier,
                &issued.resources,
                issued.issued_at,
                issued.expires_at,
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(json!({
            "success": true,
            "token": issued.token,
            "jti": issued.jti,
            "tier": tier,
            "tierLabel": tier_label(&tier),
            "expiresAt": issued.expires_at.to_rfc3339(),
            "service_id": service_id,
            "resources": resources,
        }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_revoke(state: Arc<AppState>, body_text: String) -> Response<Body> {
    let result = async {
        let body: RevokeBody = serde_json::from_str(&body_text)
            .map_err(|e| Error::BadRequest(format!("invalid json: {e}")))?;

        let wallet_line = body
            .signed
            .message
            .lines()
            .find_map(|l| l.strip_prefix("wallet: "))
            .ok_or_else(|| Error::BadRequest("wallet missing in message".into()))?;

        let parsed = verify_and_consume(
            &state,
            wallet_line,
            &body.signed.message,
            &body.signed.signature,
            Action::Revoke,
        )
        .await?;

        let service_id = parsed
            .service_id
            .ok_or_else(|| Error::BadRequest("service_id required".into()))?;
        let jti_str = parsed
            .jti
            .ok_or_else(|| Error::BadRequest("jti required".into()))?;
        let jti = Uuid::parse_str(&jti_str)
            .map_err(|e| Error::BadRequest(format!("invalid jti: {e}")))?;

        let revoked = state
            .require_db()?
            .revoke_token(jti, &service_id, wallet_line)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        if !revoked {
            return Err(Error::NotFound("token not found or already revoked".into()));
        }

        Ok(json!({ "success": true, "jti": jti_str, "revoked": true }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_introspect(state: Arc<AppState>, auth_header: Option<&str>) -> Response<Body> {
    let result = async {
        let token = decode_bearer_token(auth_header)?;
        let claims = decode_unverified_claims(&token)?;
        let jti = Uuid::parse_str(&claims.jti)
            .map_err(|e| Error::BadRequest(format!("invalid jti: {e}")))?;

        let now = Utc::now().timestamp();
        let expired = now >= claims.exp;

        let mut revoked = false;
        if let Ok(db) = state.require_db() {
            if let Some(row) = db
                .get_token(jti)
                .await
                .map_err(|e| Error::Internal(e.to_string()))?
            {
                revoked = row.revoked_at.is_some();
            }
        }

        let active = !expired && !revoked;
        Ok(json!({
            "active": active,
            "jti": claims.jti,
            "sub": claims.sub,
            "payer": claims.payer,
            "tier": claims.tier,
            "exp": claims.exp,
            "revoked": revoked,
        }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_revocations(state: Arc<AppState>, query: &str) -> Response<Body> {
    let result = async {
        let map = crate::http_util::parse_query_map(query);
        let service_id = map
            .get("service_id")
            .ok_or_else(|| Error::BadRequest("service_id required".into()))?;

        let since = map
            .get("since")
            .map(|s| {
                DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| Error::BadRequest(format!("invalid since: {e}")))
            })
            .transpose()?;

        let window_start = Utc::now() - chrono::Duration::days(state.config.revocation_window_days);

        let db = state.require_db()?;
        let (revoked_jti, cursor, complete) = db
            .list_revocations(service_id, since, window_start, 500)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(json!({
            "service_id": service_id,
            "cursor": cursor.to_rfc3339(),
            "revoked_jti": revoked_jti,
            "complete": complete,
        }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_list_subscriptions(
    state: Arc<AppState>,
    wallet: String,
    query: &str,
) -> Response<Body> {
    let result = async {
        let map = crate::http_util::parse_query_map(query);
        let service_id = map.get("service_id").map(|s| s.as_str());
        let message = map.get("message");
        let signature = map.get("signature");
        if message.is_none() || signature.is_none() {
            return Err(Error::Unauthorized(
                "wallet-signed message and signature required".into(),
            ));
        }
        let parsed = challenge_auth::verify_challenge_submission(
            &state.config.hmac_secret,
            &wallet,
            message.unwrap(),
            signature.unwrap(),
        )
        .map_err(Error::Unauthorized)?;
        wallet_matches_path(&wallet, &parsed.wallet)?;

        let rows = state
            .require_db()?
            .list_subscriptions(&wallet, service_id, 100)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let items: Vec<Value> = rows
            .into_iter()
            .map(|r| {
                json!({
                    "jti": r.jti,
                    "service_id": r.service_id,
                    "payer": r.payer,
                    "tier": r.tier,
                    "resources": r.resources,
                    "issued_at": r.issued_at.to_rfc3339(),
                    "expires_at": r.expires_at.to_rfc3339(),
                    "revoked_at": r.revoked_at.map(|t| t.to_rfc3339()),
                })
            })
            .collect();

        Ok(json!({ "subscriptions": items }))
    }
    .await;

    into_vercel_response(result)
}

pub fn route_options() -> Response<Body> {
    cors_options()
}

pub fn parse_service_wallet(path: &str, suffix: &str) -> Option<String> {
    parse_wallet_path(path, suffix)
}
