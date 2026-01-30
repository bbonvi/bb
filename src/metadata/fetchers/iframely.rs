use crate::config::ScrapeConfig;
use crate::metadata::fetchers::{fetch_bytes, MetadataFetcher};
use crate::metadata::types::Metadata;
use serde_json::Value;

pub struct IframelyFetcher;

impl IframelyFetcher {
    pub fn new() -> Self {
        Self
    }

    fn extract_metadata(resp: &Value) -> Option<IframelyResult> {
        let meta = resp.get("meta");

        let title = meta
            .and_then(|m| m.get("title"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let description = meta
            .and_then(|m| m.get("description"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let canonical_url = resp
            .get("url")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let icon_url = resp
            .get("links")
            .and_then(|l| l.get("icon"))
            .and_then(|arr| arr.as_array())
            .and_then(|arr| arr.first())
            .and_then(|obj| obj.get("href"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let image_url = resp
            .get("links")
            .and_then(|l| l.get("thumbnail"))
            .and_then(|arr| arr.as_array())
            .and_then(|arr| arr.first())
            .and_then(|obj| obj.get("href"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        if title.is_some() || description.is_some() || image_url.is_some() || icon_url.is_some() {
            Some(IframelyResult {
                title,
                description,
                canonical_url,
                icon_url,
                image_url,
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct IframelyResult {
    title: Option<String>,
    description: Option<String>,
    canonical_url: Option<String>,
    icon_url: Option<String>,
    image_url: Option<String>,
}

impl MetadataFetcher for IframelyFetcher {
    fn fetch(&self, url: &str, scrape_config: Option<&ScrapeConfig>) -> anyhow::Result<Option<Metadata>> {
        let api_key = match std::env::var("IFRAMELY_API_KEY") {
            Ok(key) if !key.is_empty() => key,
            _ => {
                log::debug!("IFRAMELY_API_KEY not set; skipping Iframely fetcher");
                return Ok(None);
            }
        };

        let client = reqwest::blocking::Client::new();
        let resp = client
            .get("https://iframe.ly/api/iframely")
            .query(&[("url", url), ("api_key", &api_key)])
            .send()?
            .json::<Value>()?;

        if let Some(error) = resp.get("error").and_then(|v| v.as_str()) {
            let status = resp.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
            log::warn!("iframely error: status={status} error={error}");
            return Ok(None);
        }

        let result = match Self::extract_metadata(&resp) {
            Some(r) => r,
            None => return Ok(None),
        };

        let mut meta = Metadata {
            title: result.title,
            description: result.description,
            canonical_url: result.canonical_url,
            icon_url: result.icon_url.clone(),
            image_url: result.image_url.clone(),
            ..Default::default()
        };

        if let Some(ref img_url) = result.image_url {
            if let Some(bytes) = fetch_bytes(img_url, scrape_config) {
                meta.image = Some(bytes);
            }
        }
        if let Some(ref icon_url) = result.icon_url {
            if let Some(bytes) = fetch_bytes(icon_url, scrape_config) {
                meta.icon = Some(bytes);
            }
        }

        Ok(Some(meta))
    }

    fn name(&self) -> &'static str {
        "Iframely"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_full_response() {
        let resp = json!({
            "url": "https://example.com/page",
            "meta": {
                "title": "Example Page",
                "description": "A description of the page",
                "author": "Author Name",
                "site": "Example"
            },
            "links": {
                "thumbnail": [{"href": "https://example.com/thumb.jpg", "media": {"width": 640, "height": 360}}],
                "icon": [{"href": "https://example.com/favicon.ico"}]
            }
        });

        let result = IframelyFetcher::extract_metadata(&resp).unwrap();
        assert_eq!(result.title.as_deref(), Some("Example Page"));
        assert_eq!(result.description.as_deref(), Some("A description of the page"));
        assert_eq!(result.canonical_url.as_deref(), Some("https://example.com/page"));
        assert_eq!(result.icon_url.as_deref(), Some("https://example.com/favicon.ico"));
        assert_eq!(result.image_url.as_deref(), Some("https://example.com/thumb.jpg"));
    }

    #[test]
    fn test_extract_missing_fields() {
        let resp = json!({
            "url": "https://example.com/page",
            "meta": {
                "title": "Only Title"
            },
            "links": {}
        });

        let result = IframelyFetcher::extract_metadata(&resp).unwrap();
        assert_eq!(result.title.as_deref(), Some("Only Title"));
        assert!(result.description.is_none());
        assert!(result.icon_url.is_none());
        assert!(result.image_url.is_none());
    }

    #[test]
    fn test_extract_no_meta() {
        let resp = json!({
            "url": "https://example.com/page"
        });

        // Has canonical_url but no title/desc/image/icon — still returns Some
        // because canonical_url alone doesn't trigger the has-data check,
        // but actually the guard checks title/desc/image/icon
        let result = IframelyFetcher::extract_metadata(&resp);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_empty_meta() {
        let resp = json!({
            "meta": {},
            "links": {}
        });

        let result = IframelyFetcher::extract_metadata(&resp);
        assert!(result.is_none());
    }
}
