use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MicrolinkResult {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub icon_url: Option<String>,  // favicon/logo
    pub image_url: Option<String>, // og:image or screenshot
}

// Optional api_key: Some("...") if you have Microlink PRO, otherwise None.
pub fn microlink(url: &str, api_key: Option<&str>) -> Option<MicrolinkResult> {
    let client = reqwest::blocking::Client::new();

    let mut req = client.get("https://api.microlink.io").query(&[
        ("url", url),
        ("screenshot", "true"),
        // Prefer highest-quality screenshot when needed.
        ("screenshot.device", "desktop"),
        ("screenshot.type", "jpeg"), // smaller than png
        ("screenshot.fullPage", "true"),
        // Ask for metadata as well.
        ("meta", "true"),
    ]);

    if let Some(key) = api_key {
        req = req.header("x-api-key", key);
    }

    let resp = req.send().ok()?.json::<Value>().ok()?;
    extract_microlink_metadata(&resp)
}

pub fn extract_microlink_metadata(resp: &Value) -> Option<MicrolinkResult> {
    // Microlink shape: { status: "success" | "error", data: { ... } }
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

    // favicon / logo
    // Prefer favicon when present; Microlink commonly exposes "logo.url"
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

    // Primary image: og:image; fallback to screenshot.url (since we request it)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_success_minimal() {
        let data = json!({
            "status": "success",
            "data": {}
        });
        let meta = extract_microlink_metadata(&data).unwrap();
        assert!(meta.title.is_none());
        assert!(meta.description.is_none());
        assert!(meta.canonical_url.is_none());
        assert!(meta.icon_url.is_none());
        assert!(meta.image_url.is_none());
    }

    #[test]
    fn test_success_full_fields() {
        let data = json!({
            "status": "success",
            "data": {
                "url": "https://example.com",
                "title": "Example Title",
                "description": "Example Description",
                "logo": { "url": "https://example.com/favicon.ico" },
                "image": { "url": "https://example.com/og.png" },
                "screenshot": { "url": "https://cdn.microlink.io/screenshot.jpg" }
            }
        });
        let meta = extract_microlink_metadata(&data).unwrap();
        assert_eq!(meta.canonical_url.as_deref(), Some("https://example.com"));
        assert_eq!(meta.title.as_deref(), Some("Example Title"));
        assert_eq!(meta.description.as_deref(), Some("Example Description"));
        assert_eq!(
            meta.icon_url.as_deref(),
            Some("https://example.com/favicon.ico")
        );
        assert_eq!(
            meta.image_url.as_deref(),
            Some("https://example.com/og.png")
        );
    }

    #[test]
    fn test_fallback_to_screenshot() {
        let data = json!({
            "status": "success",
            "data": {
                "screenshot": { "url": "https://cdn.microlink.io/screenshot.jpg" }
            }
        });
        let meta = extract_microlink_metadata(&data).unwrap();
        assert_eq!(
            meta.image_url.as_deref(),
            Some("https://cdn.microlink.io/screenshot.jpg")
        );
    }

    #[test]
    fn test_error_status() {
        let data = json!({
            "status": "error",
            "message": "Something went wrong"
        });
        assert!(extract_microlink_metadata(&data).is_none());
    }
}
