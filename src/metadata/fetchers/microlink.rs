use crate::metadata::types::Metadata;
use crate::metadata::fetchers::{MetadataFetcher, fetch_bytes};
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
        ]);

        if let Some(key) = api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req.send().ok()?.json::<Value>().ok()?;
        Self::extract_microlink_metadata(&resp)
    }

    fn extract_microlink_metadata(resp: &Value) -> Option<MicrolinkResult> {
        if resp.get("status")?.as_str()? != "success" {
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
    fn fetch(&self, url: &str) -> anyhow::Result<Option<Metadata>> {
        let api_key = std::env::var("MICROLINK_API_KEY").ok();
        if api_key.is_none() {
            log::warn!(
                "MICROLINK_API_KEY is missing; using public API (rate/feature limits may apply)"
            );
        }

        let mic_result = Self::microlink(url, api_key.as_deref());
        log::info!("microlink result: {:#?}", mic_result);

        if let Some(m) = mic_result {
            // require at least title and image_url
            if m.title.is_some() && m.image_url.is_some() {
                let mut meta = Metadata {
                    title: m.title,
                    description: m.description,
                    canonical_url: m.canonical_url,
                    icon_url: m.icon_url.clone(),
                    image_url: m.image_url.clone(),
                    keywords: None,
                    dump: None,
                    image: None,
                    icon: None,
                };
                
                // fetch image and icon
                if let Some(img_url) = m.image_url {
                    if let Some(bytes) = fetch_bytes(&img_url) {
                        meta.image = Some(bytes);
                    }
                }
                if let Some(icon_url) = m.icon_url {
                    if let Some(bytes) = fetch_bytes(&icon_url) {
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
