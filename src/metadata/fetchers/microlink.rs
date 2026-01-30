use crate::config::ScrapeConfig;
use crate::metadata::fetchers::{fetch_bytes, MetadataFetcher};
use crate::metadata::types::Metadata;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MicrolinkResult {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub icon_url: Option<String>,  // favicon/logo
    pub image_url: Option<String>, // og:image or screenshot
}

pub struct MicrolinkFetcher;

impl MicrolinkFetcher {
    pub fn new() -> Self {
        Self
    }

    pub fn microlink(url: &str, api_key: Option<&str>) -> Option<MicrolinkResult> {
        let client = reqwest::blocking::Client::new();

        let mut req = client.get("https://api.microlink.io").query(&[
            ("url", url),
            ("screenshot", "true"),
            ("screenshot.device", "desktop"),
            ("screenshot.type", "jpeg"),
            ("screenshot.fullPage", "true"),
            ("meta", "true"),
            ("adblock", "true"),
            ("device", "Macbook Pro 13"),
            ("retry", "3"),
        ]);

        if let Some(key) = api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req.send().ok()?.json::<Value>().ok()?;
        Self::extract_microlink_metadata(&resp)
    }

    fn extract_microlink_metadata(resp: &Value) -> Option<MicrolinkResult> {
        let status = resp.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
        if status != "success" {
            let message = resp.get("message").and_then(|v| v.as_str()).unwrap_or("unknown");
            log::warn!("microlink error: status={status} message={message}");
            return None;
        }
        let data = resp.get("data")?;

        let title = data
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let description = data
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let canonical_url = data.get("url").and_then(|v| v.as_str()).map(str::to_owned);

        let icon_url = data
            .get("logo")
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| {
                data.get("favicon")
                    .and_then(|v| v.get("url"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            });

        let image_url = data
            .get("image")
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| {
                data.get("screenshot")
                    .and_then(|v| v.get("url"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            });

        Some(MicrolinkResult {
            title,
            description,
            canonical_url,
            icon_url,
            image_url,
        })
    }
}

impl MetadataFetcher for MicrolinkFetcher {
    fn fetch(&self, url: &str, scrape_config: Option<&ScrapeConfig>) -> anyhow::Result<Option<Metadata>> {
        let api_key = std::env::var("MICROLINK_API_KEY").ok();
        if api_key.is_none() {
            log::warn!(
                "MICROLINK_API_KEY is missing; using public API (rate/feature limits may apply)"
            );
        }

        let mic_result = Self::microlink(url, api_key.as_deref());

        if let Some(m) = mic_result {
            // Accept partial results — any useful field is sufficient
            if m.title.is_some() || m.description.is_some() || m.image_url.is_some() || m.icon_url.is_some() {
                let mut meta = Metadata {
                    title: m.title,
                    description: m.description,
                    canonical_url: m.canonical_url,
                    icon_url: m.icon_url.clone(),
                    image_url: m.image_url.clone(),
                    ..Default::default()
                };

                // fetch image and icon
                if let Some(img_url) = m.image_url {
                    if let Some(bytes) = fetch_bytes(&img_url, scrape_config) {
                        meta.image = Some(bytes);
                    }
                }
                if let Some(icon_url) = m.icon_url {
                    if let Some(bytes) = fetch_bytes(&icon_url, scrape_config) {
                        meta.icon = Some(bytes);
                    }
                }

                return Ok(Some(meta));
            }
        }

        Ok(None)
    }

    fn name(&self) -> &'static str {
        "Microlink"
    }
}
