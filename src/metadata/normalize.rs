use url::Url;

/// Normalize a URL by removing tracking parameters, trailing slashes, and lowercasing the host.
///
/// Applies the following transformations:
/// - Strips known tracking query parameters (utm_*, fbclid, gclid, ref, mc_*)
/// - Removes trailing slashes from the path (preserves root `/`)
/// - Lowercases the hostname
/// - Converts protocol-relative URLs to https
///
/// Returns the original string if the URL cannot be parsed.
pub fn normalize_url(url: &str) -> String {
    // Handle protocol-relative URLs
    let url_to_parse = if url.starts_with("//") {
        format!("https:{}", url)
    } else {
        url.to_string()
    };

    // Parse the URL
    let mut parsed = match Url::parse(&url_to_parse) {
        Ok(u) => u,
        Err(_) => return url.to_string(), // Return original if malformed
    };

    // Lowercase the host
    if let Some(host) = parsed.host_str() {
        let lowercased = host.to_lowercase();
        if parsed.set_host(Some(&lowercased)).is_err() {
            return url.to_string();
        }
    }

    // Strip tracking query parameters
    let tracking_params = [
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "fbclid",
        "gclid",
        "ref",
        "mc_cid",
        "mc_eid",
    ];

    let filtered_params: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| !tracking_params.contains(&key.as_ref()))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Clear and rebuild query string
    parsed.set_query(None);
    if !filtered_params.is_empty() {
        let query_string = filtered_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        parsed.set_query(Some(&query_string));
    }

    // Strip trailing slashes from path (but keep root `/`)
    let path = parsed.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        let trimmed = path.trim_end_matches('/');
        parsed.set_path(trimmed);
    }

    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_utm_params() {
        let input = "https://example.com/page?utm_source=twitter&utm_medium=social&utm_campaign=spring";
        let expected = "https://example.com/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_strip_fbclid_gclid() {
        let input = "https://example.com/page?fbclid=abc123&gclid=xyz789";
        let expected = "https://example.com/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_strip_ref_and_mc_params() {
        let input = "https://example.com/page?ref=share&mc_cid=campaign123&mc_eid=email456";
        let expected = "https://example.com/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_trailing_slash_removal() {
        let input = "https://example.com/page/";
        let expected = "https://example.com/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_trailing_slash_removal_nested() {
        let input = "https://example.com/path/to/page/";
        let expected = "https://example.com/path/to/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_preserve_root_slash() {
        let input = "https://example.com/";
        let expected = "https://example.com/";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_lowercase_host() {
        let input = "https://EXAMPLE.COM/Page";
        let expected = "https://example.com/Page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_lowercase_host_mixed_case() {
        let input = "https://ExAmPlE.CoM/path";
        let expected = "https://example.com/path";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_protocol_relative_url() {
        let input = "//example.com/page";
        let expected = "https://example.com/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_protocol_relative_url_with_params() {
        let input = "//example.com/page?utm_source=test&foo=bar";
        let expected = "https://example.com/page?foo=bar";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_preserve_non_tracking_params() {
        let input = "https://example.com/page?foo=bar&baz=qux&utm_source=removed";
        let expected = "https://example.com/page?foo=bar&baz=qux";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_preserve_non_tracking_params_only() {
        let input = "https://example.com/page?search=query&page=2";
        let expected = "https://example.com/page?search=query&page=2";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_url_without_query_string() {
        let input = "https://example.com/page";
        let expected = "https://example.com/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_url_with_fragment() {
        let input = "https://example.com/page?utm_source=test#section";
        let expected = "https://example.com/page#section";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_malformed_url_returns_original() {
        let input = "not a valid url";
        let expected = "not a valid url";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_malformed_url_missing_scheme() {
        let input = "example.com/page";
        let expected = "example.com/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_combined_normalizations() {
        let input = "https://EXAMPLE.COM/Path/To/Page/?utm_source=test&foo=bar&fbclid=123";
        let expected = "https://example.com/Path/To/Page?foo=bar";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_all_params_removed_no_query() {
        let input = "https://example.com/page?utm_source=test&utm_medium=email";
        let expected = "https://example.com/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_port_preserved() {
        let input = "https://example.com:8080/page?utm_source=test";
        let expected = "https://example.com:8080/page";
        assert_eq!(normalize_url(input), expected);
    }

    #[test]
    fn test_subdomain_lowercase() {
        let input = "https://WWW.EXAMPLE.COM/page";
        let expected = "https://www.example.com/page";
        assert_eq!(normalize_url(input), expected);
    }
}
