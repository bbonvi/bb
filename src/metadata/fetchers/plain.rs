use crate::config::ScrapeConfig;
use crate::metadata::types::{Metadata, MetaOptions};
use crate::metadata::fetchers::{MetadataFetcher, fetch_bytes};

pub struct PlainFetcher;

impl PlainFetcher {
    pub fn new() -> Self {
        Self
    }
}

impl MetadataFetcher for PlainFetcher {
    fn fetch(&self, url: &str, scrape_config: Option<&ScrapeConfig>) -> anyhow::Result<Option<Metadata>> {
        // Try basic reqwest fetch first
        if let Some(reqwest_result) = crate::scrape::fetch_page_with_reqwest(url, scrape_config) {
            log::debug!("plain request successful");
            let mut meta = crate::scrape::get_data_from_page(reqwest_result.html.clone(), url);

            // Try fetching image if present
            if meta.image.is_none() {
                meta.try_fetch_image(scrape_config);
            }

            // Try fetching icon if present
            if meta.icon.is_none() {
                meta.try_fetch_icon(scrape_config);
            }

            return Ok(Some(meta));
        }

        Ok(None)
    }
    
    fn name(&self) -> &'static str {
        "Plain"
    }

    fn priority(&self) -> u8 {
        2 // Second priority after oEmbed
    }
}

/// Enhanced plain fetcher that can use headless browser if needed
pub struct HeadlessFetcher {
    opts: MetaOptions,
}

impl HeadlessFetcher {
    pub fn new(opts: MetaOptions) -> Self {
        Self { opts }
    }
    
    pub fn fetch_with_headless(&self, url: &str, scrape_config: Option<&crate::config::ScrapeConfig>) -> anyhow::Result<Option<Metadata>> {
        if self.opts.no_headless {
            return Ok(None);
        }
        
        #[cfg(feature = "headless")]
        {
            if let Some(chrome_res) = crate::scrape::headless::fetch_page_with_chrome(url) {
                let blocked = chrome_res.html.contains("Sorry, you have been blocked")
                    || chrome_res.html.contains("Verify you are human by completing the action below");
                
                if !blocked {
                    let mut meta = crate::scrape::get_data_from_page(chrome_res.html.clone(), url);
                    
                    // Try YouTube thumbnail first if no image
                    if meta.image.is_none() {
                        if let Some(y_img) = self.get_youtube_image_url(url) {
                            if let Some(bytes) = fetch_bytes(&y_img, scrape_config) {
                                meta.image = Some(bytes);
                                meta.image_url = Some(y_img.clone());
                            }
                        }
                    }
                    
                    // If still no image, use screenshot
                    if meta.image.is_none() {
                        meta.image = Some(chrome_res.screenshot);
                    }
                    
                    // Try icon
                    if meta.icon.is_none() {
                        meta.try_fetch_icon(scrape_config);
                    }
                    
                    return Ok(Some(meta));
                }
            }
        }
        
        Ok(None)
    }
    
    fn get_youtube_image_url(&self, url: &str) -> Option<String> {
        // This could be moved to a shared utility or the YouTube fetcher
        use regex::Regex;
        use once_cell::sync::Lazy;
        
        static YOUTUBE_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(
                r"(?:https?://)?(?:www\.)?(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/shorts/)([A-Za-z0-9_-]{11})",
            )
            .expect("Failed to compile YouTube regex")
        });
        
        YOUTUBE_REGEX
            .captures(url)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_owned()))
            .map(|video_id| format!("https://img.youtube.com/vi/{}/maxresdefault.jpg", video_id))
    }
}
