use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub canonical_url: Option<String>,
    pub icon_url: Option<String>,
    pub image_url: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub image: Option<Vec<u8>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub icon: Option<Vec<u8>>,
    pub dump: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetaOptions {
    pub no_headless: bool,
}

impl Metadata {
    /// Check if the metadata is essentially empty (no useful fields)
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.icon_url.is_none()
            && self.image_url.is_none()
            && self.keywords.is_none()
            && self.canonical_url.is_none()
    }

    /// Try to fetch and set image bytes from image_url if present
    pub fn try_fetch_image(&mut self) {
        if let Some(ref img_url) = self.image_url {
            if let Some(bytes) = crate::metadata::fetchers::fetch_bytes(img_url) {
                self.image = Some(bytes);
            }
        }
    }

    /// Try to fetch and set icon bytes from icon_url if present
    pub fn try_fetch_icon(&mut self) {
        if let Some(ref icon_url) = self.icon_url {
            if let Some(bytes) = crate::metadata::fetchers::fetch_bytes(icon_url) {
                self.icon = Some(bytes);
            }
        }
    }
}
