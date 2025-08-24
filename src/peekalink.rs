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

    let metadata = extract_metadata_from_value(&resp)?;

    log::info!("m: {:?}", metadata);

    if metadata.title.is_some() && metadata.image_url.is_some() {
        Some(metadata)
    } else {
        None
    }
}

pub fn extract_metadata_from_value(resp: &Value) -> Option<PeekalinkResult> {
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

    let image_url = image_from_nested(
        &resp,
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
        image_from_nested(
            &resp,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_minimal_ok() {
        let data = json!({
            "ok": true
        });
        let meta = extract_metadata_from_value(&data).unwrap();
        assert!(meta.title.is_none());
        assert!(meta.description.is_none());
        assert!(meta.canonical_url.is_none());
        assert!(meta.icon_url.is_none());
        assert!(meta.image_url.is_none());
    }

    #[test]
    fn test_full_fields() {
        let data = json!({
            "ok": true,
            "url": "https://real.site/",
            "title": "Title Here",
            "description": "Desc.",
            "icon": { "original": { "url": "https://icon.url/icon.png" } },
            "image": { "original": "https://image.url/img.png" }
        });
        let meta = extract_metadata_from_value(&data).unwrap();
        assert_eq!(meta.canonical_url.as_deref(), Some("https://real.site/"));
        assert_eq!(meta.title.as_deref(), Some("Title Here"));
        assert_eq!(meta.description.as_deref(), Some("Desc."));
        assert_eq!(meta.icon_url.as_deref(), Some("https://icon.url/icon.png"));
        // Image found in generic location
        assert_eq!(meta.image_url.as_deref(), Some("https://image.url/img.png"));
    }

    #[test]
    fn test_youtube_priority() {
        let data = json!({
            "ok": true,
            "youtubeVideo": {
                "thumbnail": { "original": { "url": "https://yt/thumb.jpg" } }
            }
        });
        let meta = extract_metadata_from_value(&data).unwrap();
        // Finds YouTube video image
        assert_eq!(meta.image_url.as_deref(), Some("https://yt/thumb.jpg"));
    }

    #[test]
    fn test_missing_ok() {
        let data = json!({
            "title": "stuff"
        });
        assert!(extract_metadata_from_value(&data).is_none());
    }

    #[test]
    fn test_ok_false() {
        let data = json!({
            "ok": false,
            "title": "stuff"
        });
        assert!(extract_metadata_from_value(&data).is_none());
    }

    #[test]
    fn test_instagram_priority() {
        let data = json!({
            "ok": true,
            "instagramPost": {
                "media": {
                    "0": {
                        "original": { "url": "https://instagram/test.jpg" }
                    }
                }
            }
        });
        let meta = extract_metadata_from_value(&data).unwrap();
        assert_eq!(
            meta.image_url.as_deref(),
            Some("https://instagram/test.jpg")
        );
    }
}
