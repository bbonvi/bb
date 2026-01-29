use crate::config::ScrapeConfig;
use crate::metadata::fetchers::MetadataFetcher;
use crate::metadata::types::Metadata;

pub struct WaybackFetcher;

impl WaybackFetcher {
    pub fn new() -> Self {
        Self
    }
}

impl MetadataFetcher for WaybackFetcher {
    fn fetch(
        &self,
        url: &str,
        scrape_config: Option<&ScrapeConfig>,
    ) -> anyhow::Result<Option<Metadata>> {
        let snapshot_url = find_snapshot(url, scrape_config)?;
        let snapshot_url = match snapshot_url {
            Some(u) => u,
            None => return Ok(None),
        };

        let result = crate::scrape::fetch_page_with_reqwest(&snapshot_url, scrape_config);
        let html = match result {
            Some(r) => r.html,
            None => return Ok(None),
        };

        // Parse with ORIGINAL url for relative URL resolution (not archive.org)
        let mut meta = crate::scrape::get_data_from_page(html, url);

        // Clear icon_url — archived pages have rewritten links to archive.org
        meta.icon_url = None;

        if meta.title.is_some() || meta.description.is_some() || meta.image_url.is_some() {
            Ok(Some(meta))
        } else {
            Ok(None)
        }
    }

    fn name(&self) -> &'static str {
        "Wayback"
    }
}

fn find_snapshot(
    url: &str,
    scrape_config: Option<&ScrapeConfig>,
) -> anyhow::Result<Option<String>> {
    let api_url = format!("https://archive.org/wayback/available?url={}", url);
    let (status, bytes) = match crate::scrape::reqwest_with_retries(&api_url, scrape_config) {
        Some(r) => r,
        None => return Ok(None),
    };
    if !status.is_success() {
        return Ok(None);
    }

    let json: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok(parse_snapshot_url(&json).map(|s| s.to_string()))
}

fn parse_snapshot_url(json: &serde_json::Value) -> Option<&str> {
    let closest = json.get("archived_snapshots")?.get("closest")?;
    let available = closest
        .get("available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let status = closest
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if available && status == "200" {
        closest.get("url").and_then(|v| v.as_str())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_snapshot() {
        let json = serde_json::json!({
            "archived_snapshots": {
                "closest": {
                    "available": true,
                    "url": "http://web.archive.org/web/20230101/http://example.com/",
                    "timestamp": "20230101000000",
                    "status": "200"
                }
            }
        });
        assert_eq!(
            parse_snapshot_url(&json),
            Some("http://web.archive.org/web/20230101/http://example.com/")
        );
    }

    #[test]
    fn test_parse_empty_snapshots() {
        let json = serde_json::json!({"archived_snapshots": {}});
        assert_eq!(parse_snapshot_url(&json), None);
    }

    #[test]
    fn test_parse_non_200_snapshot() {
        let json = serde_json::json!({
            "archived_snapshots": {
                "closest": {
                    "available": true,
                    "url": "http://web.archive.org/web/20230101/http://example.com/",
                    "timestamp": "20230101000000",
                    "status": "301"
                }
            }
        });
        assert_eq!(parse_snapshot_url(&json), None);
    }

    #[test]
    fn test_parse_unavailable_snapshot() {
        let json = serde_json::json!({
            "archived_snapshots": {
                "closest": {
                    "available": false,
                    "url": "http://web.archive.org/web/20230101/http://example.com/",
                    "timestamp": "20230101000000",
                    "status": "200"
                }
            }
        });
        assert_eq!(parse_snapshot_url(&json), None);
    }
}
