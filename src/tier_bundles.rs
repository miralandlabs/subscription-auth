//! JSON validation for subscription catalog metadata (`tier_bundles`).

use {
    crate::error::Error,
    serde_json::{json, Value},
};

const MAX_TAGS: usize = 10;
const MAX_TAG_LEN: usize = 64;

const CATEGORIES: &[&str] = &[
    "social-media",
    "blockchain",
    "ai-ml",
    "finance",
    "developer-tools",
    "content",
    "communication",
    "other",
];

/// Validate optional `tier_bundles` on register/update. `None` is allowed.
pub fn validate_tier_bundles(value: Option<&Value>) -> Result<(), Error> {
    let Some(v) = value else {
        return Ok(());
    };
    if !v.is_object() {
        return Err(Error::BadRequest(
            "tier_bundles must be a JSON object".into(),
        ));
    }
    validate_display(v.get("display"))?;
    validate_tiers(v.get("tiers"))?;
    if let Some(c) = v.get("compliance") {
        validate_object(c, "compliance")?;
    }
    if let Some(s) = v.get("sla") {
        validate_object(s, "sla")?;
    }
    Ok(())
}

/// Extract indexed category and normalized tags from validated tier_bundles.
pub fn extract_catalog_index(value: Option<&Value>) -> (Option<String>, Value) {
    let Some(v) = value else {
        return (None, json!([]));
    };
    let display = match v.get("display").and_then(|d| d.as_object()) {
        Some(d) => d,
        None => return (None, json!([])),
    };
    let category = display
        .get("category")
        .and_then(|c| c.as_str())
        .map(String::from);
    let tags = normalize_tags(display.get("tags"));
    (category, tags)
}

pub fn pricing_summary(tier_bundles: Option<&Value>) -> Value {
    let Some(v) = tier_bundles else {
        return json!({
            "configured": false,
            "starting_at_usdc": null,
            "has_free_tier": false,
            "billing_models": []
        });
    };
    let tiers = match v.get("tiers").and_then(|t| t.as_array()) {
        Some(t) if !t.is_empty() => t,
        _ => {
            return json!({
                "configured": display_name(Some(v)).is_some(),
                "starting_at_usdc": null,
                "has_free_tier": false,
                "billing_models": []
            });
        }
    };

    let mut min_monthly: Option<f64> = None;
    let mut has_free = false;
    let mut billing = Vec::new();

    for tier in tiers {
        let month = tier.get("price_usdc_per_month").and_then(|p| p.as_f64());
        let req = tier.get("price_usdc_per_request").and_then(|p| p.as_f64());
        if month == Some(0.0) || req == Some(0.0) {
            has_free = true;
        }
        if let Some(m) = month {
            min_monthly = Some(min_monthly.map_or(m, |cur| cur.min(m)));
            if !billing.contains(&"subscription") {
                billing.push("subscription");
            }
        }
        if let Some(r) = req {
            if r > 0.0 && !billing.contains(&"per_request") {
                billing.push("per_request");
            }
            let _ = r;
        }
    }

    json!({
        "configured": true,
        "starting_at_usdc": min_monthly,
        "has_free_tier": has_free,
        "billing_models": billing
    })
}

pub fn display_block(tier_bundles: Option<&Value>) -> Option<Value> {
    tier_bundles
        .and_then(|v| v.get("display"))
        .filter(|d| d.is_object())
        .cloned()
}

pub fn display_name(tier_bundles: Option<&Value>) -> Option<String> {
    tier_bundles
        .and_then(|v| v.get("display"))
        .and_then(|d| d.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from)
}

fn validate_display(display: Option<&Value>) -> Result<(), Error> {
    let Some(d) = display else {
        return Err(Error::BadRequest("tier_bundles.display is required".into()));
    };
    let obj = validate_object(d, "display")?;
    let name = obj
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| Error::BadRequest("tier_bundles.display.name is required".into()))?;
    if name.is_empty() || name.len() > 200 {
        return Err(Error::BadRequest(
            "tier_bundles.display.name must be 1-200 chars".into(),
        ));
    }
    reject_html(name, "tier_bundles.display.name")?;

    if let Some(tagline) = obj.get("tagline").and_then(|t| t.as_str()) {
        reject_html(tagline, "tier_bundles.display.tagline")?;
    }
    if let Some(desc) = obj.get("description").and_then(|t| t.as_str()) {
        reject_html(desc, "tier_bundles.display.description")?;
    }
    if let Some(icon) = obj.get("icon_url").and_then(|t| t.as_str()) {
        if !icon.starts_with("https://") {
            return Err(Error::BadRequest(
                "tier_bundles.display.icon_url must be https".into(),
            ));
        }
    }

    let category = obj
        .get("category")
        .and_then(|c| c.as_str())
        .ok_or_else(|| Error::BadRequest("tier_bundles.display.category is required".into()))?;
    if !CATEGORIES.contains(&category) {
        return Err(Error::BadRequest(format!(
            "tier_bundles.display.category must be one of: {}",
            CATEGORIES.join(", ")
        )));
    }

    if let Some(tags) = obj.get("tags") {
        validate_tags_array(tags)?;
    }

    Ok(())
}

fn validate_tiers(tiers: Option<&Value>) -> Result<(), Error> {
    let Some(arr) = tiers.and_then(|t| t.as_array()) else {
        return Err(Error::BadRequest(
            "tier_bundles.tiers must be a non-empty array".into(),
        ));
    };
    if arr.is_empty() {
        return Err(Error::BadRequest(
            "tier_bundles.tiers must not be empty".into(),
        ));
    }
    for (i, tier) in arr.iter().enumerate() {
        let obj = tier
            .as_object()
            .ok_or_else(|| Error::BadRequest(format!("tier_bundles.tiers[{i}] must be object")))?;
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::BadRequest(format!("tier_bundles.tiers[{i}].id required")))?;
        if id.is_empty() || id.len() > 64 {
            return Err(Error::BadRequest(format!(
                "tier_bundles.tiers[{i}].id invalid length"
            )));
        }
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::BadRequest(format!("tier_bundles.tiers[{i}].name required")))?;
        reject_html(name, &format!("tier_bundles.tiers[{i}].name"))?;
        let _ = id;
    }
    Ok(())
}

fn validate_tags_array(tags: &Value) -> Result<(), Error> {
    let arr = tags
        .as_array()
        .ok_or_else(|| Error::BadRequest("tier_bundles.display.tags must be array".into()))?;
    if arr.len() > MAX_TAGS {
        return Err(Error::BadRequest(format!(
            "tier_bundles.display.tags max {MAX_TAGS} items"
        )));
    }
    for t in arr {
        let s = t.as_str().ok_or_else(|| {
            Error::BadRequest("tier_bundles.display.tags items must be strings".into())
        })?;
        if s.is_empty() || s.len() > MAX_TAG_LEN {
            return Err(Error::BadRequest(
                "tier_bundles.display.tags item length invalid".into(),
            ));
        }
        reject_html(s, "tier_bundles.display.tags")?;
    }
    Ok(())
}

fn normalize_tags(tags: Option<&Value>) -> Value {
    let Some(arr) = tags.and_then(|t| t.as_array()) else {
        return json!([]);
    };
    let mut out: Vec<String> = arr
        .iter()
        .filter_map(|t| t.as_str())
        .map(|s| s.to_lowercase())
        .take(MAX_TAGS)
        .collect();
    out.sort();
    out.dedup();
    json!(out)
}

fn validate_object<'a>(
    v: &'a Value,
    label: &str,
) -> Result<&'a serde_json::Map<String, Value>, Error> {
    v.as_object()
        .ok_or_else(|| Error::BadRequest(format!("{label} must be object")))
}

fn reject_html(s: &str, field: &str) -> Result<(), Error> {
    let lower = s.to_lowercase();
    if lower.contains("<script") || lower.contains("</script") || lower.contains("javascript:") {
        return Err(Error::BadRequest(format!(
            "{field} contains disallowed content"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_minimal_tier_bundles() {
        let v = json!({
            "display": {
                "name": "Demo API",
                "category": "developer-tools",
                "tags": ["api"]
            },
            "tiers": [{ "id": "free", "name": "Free" }]
        });
        assert!(validate_tier_bundles(Some(&v)).is_ok());
    }

    #[test]
    fn rejects_missing_display() {
        let v = json!({ "tiers": [{ "id": "a", "name": "A" }] });
        assert!(validate_tier_bundles(Some(&v)).is_err());
    }

    #[test]
    fn rejects_bad_category() {
        let v = json!({
            "display": { "name": "X", "category": "not-a-category" },
            "tiers": [{ "id": "a", "name": "A" }]
        });
        assert!(validate_tier_bundles(Some(&v)).is_err());
    }
}
