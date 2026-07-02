use crate::error::Error;

/// Hosted Tier B requires namespaced service_id (§9.3 design doc).
pub fn validate_service_id(service_id: &str) -> Result<(), Error> {
    let id = service_id.trim();
    if id.is_empty() {
        return Err(Error::BadRequest("service_id is required".into()));
    }
    if id.len() > 253 {
        return Err(Error::BadRequest("service_id too long".into()));
    }
    if id.contains(' ') || id.contains('/') {
        return Err(Error::BadRequest(
            "service_id must not contain spaces or slashes".into(),
        ));
    }

    // DNS-style: must contain a dot (e.g. fifa.example.com)
    if id.contains('.') {
        let parts: Vec<&str> = id.split('.').collect();
        if parts.len() >= 2 && parts.iter().all(|p| !p.is_empty()) {
            return Ok(());
        }
    }

    // Wallet-qualified: prefix before colon (e.g. 7xKX...:fifa)
    if let Some((prefix, slug)) = id.split_once(':') {
        if !prefix.is_empty() && !slug.is_empty() && slug.len() >= 2 {
            return Ok(());
        }
    }

    Err(Error::BadRequest(
        "service_id must use your domain or a wallet prefix to prevent name squatting.\n\
         Examples: \"api.myproduct.com\", \"myapp.example.com\", or \"YourWallet:myapp\"\n\
         Flat names like \"myapp\" are not allowed."
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_dns_style() {
        validate_service_id("fifa.polystrike.io").unwrap();
    }

    #[test]
    fn accepts_wallet_qualified() {
        validate_service_id("7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuZosraf:fifa").unwrap();
    }

    #[test]
    fn rejects_flat_squat() {
        assert!(validate_service_id("fifa").is_err());
    }
}
