use crate::error::Error;

pub const TIER_HOURLY: &str = "hourly";
pub const TIER_DAILY: &str = "daily";
pub const TIER_MONTHLY: &str = "monthly";

pub const ALL_TIERS: &[&str] = &[TIER_HOURLY, TIER_DAILY, TIER_MONTHLY];

pub fn tier_duration_secs(tier: &str) -> Option<i64> {
    match tier {
        TIER_HOURLY => Some(60 * 60),
        TIER_DAILY => Some(24 * 60 * 60),
        TIER_MONTHLY => Some(30 * 24 * 60 * 60),
        _ => None,
    }
}

pub fn is_valid_tier(tier: &str) -> bool {
    ALL_TIERS.contains(&tier)
}

pub fn tier_label(tier: &str) -> &'static str {
    match tier {
        TIER_HOURLY => "1 hour",
        TIER_DAILY => "24 hours",
        TIER_MONTHLY => "30 days",
        _ => "unknown",
    }
}

pub fn validate_tier(tier: &str) -> Result<(), Error> {
    if is_valid_tier(tier) {
        Ok(())
    } else {
        Err(Error::BadRequest(format!(
            "tier must be one of: {}",
            ALL_TIERS.join(", ")
        )))
    }
}
