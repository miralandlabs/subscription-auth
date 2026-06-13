use serde_json::Value;

pub fn resources_subset_of_allowlist(resources: &[String], allowlist: &Value) -> bool {
    let allow: Vec<String> = allowlist
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if allow.iter().any(|p| p == "*") {
        return true;
    }

    resources.iter().all(|r| {
        if r == "*" {
            allow.iter().any(|p| p == "*")
        } else {
            allow.contains(r)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wildcard_allowlist() {
        assert!(resources_subset_of_allowlist(
            &["/api/v1/echo".into()],
            &json!(["*"])
        ));
    }

    #[test]
    fn exact_subset() {
        assert!(resources_subset_of_allowlist(
            &["/a".into()],
            &json!(["/a", "/b"])
        ));
        assert!(!resources_subset_of_allowlist(
            &["/c".into()],
            &json!(["/a"])
        ));
    }
}
