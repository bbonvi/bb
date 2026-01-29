use crate::config::ScrapeConfig;
use crate::metadata::types::Metadata;
use crate::metadata::fetchers::{MetadataFetcher, fetch_bytes};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PeekalinkResult {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub icon_url: Option<String>,
    pub image_url: Option<String>,
}

pub struct PeekalinkFetcher;

impl PeekalinkFetcher {
    pub fn new() -> Self {
        Self
    }
    
    pub fn peekalink(url: &str, api_key: &str) -> Option<PeekalinkResult> {
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post("https://api.peekalink.io/")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&json!({ "link": url }))
            .send()
            .ok()?
            .json::<Value>()
            .ok()?;

        let metadata = Self::extract_metadata_from_value(&resp)?;

        log::info!("m: {:?}", metadata);

        if metadata.title.is_some() || metadata.description.is_some() || metadata.image_url.is_some() || metadata.icon_url.is_some() {
            Some(metadata)
        } else {
            None
        }
    }

    fn extract_metadata_from_value(resp: &Value) -> Option<PeekalinkResult> {
        log::info!("resp: {:?}", resp);
        if !resp.get("ok")?.as_bool()? {
            return None;
        }

        let canonical_url = resp
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let mut title = resp
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        // If youtubeVideo present, try to get user name and override title
        if let Some(youtube) = resp.get("youtubeVideo") {
            let user = youtube.get("user");
            let username = user
                .and_then(|u| u.get("name").and_then(|v| v.as_str()))
                .or_else(|| user.and_then(|u| u.get("username").and_then(|v| v.as_str())));
            if let (Some(video_title), Some(user_name)) = (title.clone(), username) {
                title = Some(format!("{} - {}", video_title, user_name));
            }
        }
        
        let description = resp
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let icon_url = resp
            .get("icon")
            .and_then(|icon| icon.get("original").and_then(|u| u.as_str()))
            .map(|s| s.to_owned());

        let image_url = Self::image_from_nested(
            resp,
            &[
                // YouTube
                &["youtubeVideo", "thumbnail", "original"],
                &["youtubeVideo", "thumbnail", "large"],
                &["youtubeVideo", "thumbnail", "medium"],
                &["youtubeVideo", "thumbnail", "thumbnail"],
                // Instagram
                &["instagramPost", "media", "0", "original"],
                &["instagramPost", "media", "0", "large"],
                &["instagramPost", "media", "0", "medium"],
                &["instagramPost", "media", "0", "thumbnail"],
                // X, Instagram users
                &["xUser", "avatar", "original"],
                &["instagramUser", "avatar", "original"],
                // TikTok, Reddit, etc
                &["tiktokVideo", "media", "0", "original"],
                &["tiktokUser", "avatar", "original"],
                &["redditPost", "media", "0", "original"],
                &["redditUser", "avatar", "original"],
                &["redditSubreddit", "avatar", "original"],
                // Amazon, Etsy, Github
                &["amazonProduct", "media", "0", "original"],
                &["etsyProduct", "media", "0", "original"],
            ],
        )
        .or_else(|| {
            Self::image_from_nested(
                resp,
                &[
                    &["image", "original"],
                    &["image", "large"],
                    &["image", "medium"],
                    &["image", "thumbnail"],
                    &["page", "screenshot", "original"],
                    &["page", "screenshot", "large"],
                    &["page", "screenshot", "medium"],
                    &["page", "screenshot", "thumbnail"],
                    &["document", "image", "original"],
                    &["document", "image", "large"],
                    &["document", "image", "medium"],
                    &["document", "image", "thumbnail"],
                ],
            )
        });

        Some(PeekalinkResult {
            title,
            description,
            canonical_url,
            icon_url,
            image_url,
        })
    }

    fn image_from_nested(obj: &Value, keys: &[&[&str]]) -> Option<String> {
        for key_path in keys {
            let mut current = obj;
            let mut found = true;
            for key in *key_path {
                current = match current.get(key) {
                    Some(val) => val,
                    None => {
                        found = false;
                        break;
                    }
                };
            }
            if found {
                if let Some(url) = current.get("url").and_then(|v| v.as_str()) {
                    return Some(url.to_string());
                }
                if current.is_string() {
                    return current.as_str().map(|s| s.to_string());
                }
            }
        }
        None
    }
}

impl MetadataFetcher for PeekalinkFetcher {
    fn fetch(&self, url: &str, scrape_config: Option<&ScrapeConfig>) -> anyhow::Result<Option<Metadata>> {
        let api_key = match std::env::var("PEEKALINK_API_KEY") {
            Ok(key) if !key.is_empty() => key,
            _ => {
                log::warn!("PEEKALINK_API_KEY is missing");
                return Ok(None);
            }
        };

        let peek_result = Self::peekalink(url, &api_key);
        log::info!("peekalink result: {:#?}", peek_result);

        if let Some(m) = peek_result {
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
        "Peekalink"
    }
}
