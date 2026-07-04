//! Public read-only subscription catalog for pr402-registry Subscriptions tab.

use {
    chrono::{DateTime, Utc},
    serde_json::{json, Value},
    std::sync::Arc,
    vercel_runtime::{Body, Response},
};

use crate::{
    db::MarketplaceRow,
    error::{into_vercel_response, Error},
    state::AppState,
    tier_bundles::{display_block, display_name, pricing_summary},
};

#[derive(Debug, Clone)]
pub struct MarketplaceListFilters {
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub q: Option<String>,
    pub limit: i64,
    pub cursor: Option<(DateTime<Utc>, String)>,
}

pub async fn handle_marketplace_list(state: Arc<AppState>, query: &str) -> Response<Body> {
    let result = async {
        let filters = parse_list_filters(query)?;
        let db = state.require_db()?;
        let (rows, has_more, total) = db
            .list_marketplace_subscriptions(
                filters.category.as_deref(),
                &filters.tags,
                filters.q.as_deref(),
                filters.limit,
                filters.cursor,
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let subscriptions: Vec<Value> = rows
            .iter()
            .map(marketplace_list_item)
            .collect();

        let next_cursor = if has_more {
            rows.last().map(|r| encode_cursor(r.updated_at, &r.service_id))
        } else {
            None
        };

        Ok(json!({
            "subscriptions": subscriptions,
            "pagination": {
                "next_cursor": next_cursor,
                "has_more": has_more,
                "total": total
            },
            "notice": "Advisory catalog metadata. Authoritative subscribe pricing comes from live HTTP 402 on the seller subscribe endpoint."
        }))
    }
    .await;

    into_vercel_response(result)
}

pub async fn handle_marketplace_detail(state: Arc<AppState>, service_id: String) -> Response<Body> {
    let result = async {
        crate::service_id::validate_service_id(&service_id)?;
        let db = state.require_db()?;
        let row = db
            .get_marketplace_subscription(&service_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::NotFound("subscription product not found".into()))?;

        let bundles = row.tier_bundles.as_ref();
        let display = display_block(bundles);
        let tiers = bundles.and_then(|b| b.get("tiers")).cloned();
        let compliance = bundles.and_then(|b| b.get("compliance")).cloned();
        let sla = bundles.and_then(|b| b.get("sla")).cloned();

        Ok(json!({
            "service_id": row.service_id,
            "merchant_wallet": row.merchant_wallet,
            "service_url": row.service_url,
            "status": row.status,
            "display": display,
            "tiers": tiers,
            "compliance": compliance,
            "sla": sla,
            "pricing_summary": pricing_summary(bundles),
            "resources_allowlist": row.resources_allowlist,
            "catalog_configured": display_name(bundles).is_some(),
            "created_at": row.created_at.to_rfc3339(),
            "updated_at": row.updated_at.to_rfc3339(),
            "notice": "Subscribe via the seller POST /api/v1/subscribe endpoint. Optional pr402 Resources tab listings are separate agent discovery rows."
        }))
    }
    .await;

    into_vercel_response(result)
}

fn parse_list_filters(query: &str) -> Result<MarketplaceListFilters, Error> {
    let map = crate::http_util::parse_query_map(query);
    let limit = map
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 100);

    let tags: Vec<String> = map
        .get("tags")
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let cursor = map.get("cursor").map(|c| decode_cursor(c)).transpose()?;

    Ok(MarketplaceListFilters {
        category: map.get("category").cloned(),
        tags,
        q: map.get("q").cloned(),
        limit,
        cursor,
    })
}

fn encode_cursor(updated_at: DateTime<Utc>, service_id: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let payload = format!("{}|{}", updated_at.to_rfc3339(), service_id);
    URL_SAFE_NO_PAD.encode(payload.as_bytes())
}

fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, String), Error> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| Error::BadRequest("invalid cursor".into()))?;
    let s = String::from_utf8(bytes).map_err(|_| Error::BadRequest("invalid cursor".into()))?;
    let (ts, sid) = s
        .split_once('|')
        .ok_or_else(|| Error::BadRequest("invalid cursor".into()))?;
    let dt = DateTime::parse_from_rfc3339(ts)
        .map_err(|_| Error::BadRequest("invalid cursor timestamp".into()))?
        .with_timezone(&Utc);
    Ok((dt, sid.to_string()))
}

fn marketplace_list_item(r: &MarketplaceRow) -> Value {
    json!({
        "service_id": r.service_id,
        "display": display_block(r.tier_bundles.as_ref()),
        "pricing_summary": pricing_summary(r.tier_bundles.as_ref()),
        "catalog_configured": display_name(r.tier_bundles.as_ref()).is_some(),
    })
}
