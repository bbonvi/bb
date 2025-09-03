use crate::metadata::types::Metadata;
use crate::metadata::fetchers::MetadataFetcher;
use once_cell::sync::Lazy;
use regex::Regex;

/// Compile YouTube regex once
static YOUTUBE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?:https?://)?(?:www\.)?(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/shorts/)([A-Za-z0-9_-]{11})",
    )
    .expect("Failed to compile YouTube regex")
});

pub struct YouTubeFetcher;

impl YouTubeFetcher {
    pub fn new() -> Self {
        Self
    }
    
    fn get_youtube_image_url(url: &str) -> Option<String> {
        YOUTUBE_REGEX
            .captures(url)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_owned()))
            .map(|video_id| format!("https://img.youtube.com/vi/{}/maxresdefault.jpg", video_id))
    }
}

impl MetadataFetcher for YouTubeFetcher {
    fn fetch(&self, url: &str) -> anyhow::Result<Option<Metadata>> {
        // YouTube fetcher only provides image URLs, not full metadata
        // It's used as a fallback for other fetchers
        if let Some(image_url) = Self::get_youtube_image_url(url) {
            let mut meta = Metadata::default();
            meta.image_url = Some(image_url);
            
            // Try to fetch the actual image
            meta.try_fetch_image();
            
            Ok(Some(meta))
        } else {
            Ok(None)
        }
    }
    
    fn name(&self) -> &'static str {
        "YouTube"
    }
}
