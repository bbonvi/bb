use crate::config::ScrapeConfig;
use crate::metadata::fetchers::MetadataFetcher;
use crate::metadata::types::{FetchOutcome, Metadata};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};

const PROVIDERS_URL: &str = "https://oembed.com/providers.json";
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours
const OEMBED_TIMEOUT: Duration = Duration::from_secs(5);

/// Global cache for provider list
static PROVIDER_CACHE: Lazy<RwLock<ProviderCache>> = Lazy::new(|| {
    RwLock::new(ProviderCache {
        providers: Vec::new(),
        fetched_at: None,
    })
});

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Provider {
    provider_name: String,
    provider_url: String,
    endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Endpoint {
    #[serde(default)]
    schemes: Vec<String>,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OembedResponse {
    #[serde(rename = "type")]
    oembed_type: Option<String>,
    title: Option<String>,
    author_name: Option<String>,
    provider_name: Option<String>,
    thumbnail_url: Option<String>,
    description: Option<String>,
}

struct ProviderCache {
    providers: Vec<Provider>,
    fetched_at: Option<Instant>,
}

impl ProviderCache {
    fn is_stale(&self) -> bool {
        match self.fetched_at {
            None => true,
            Some(fetched) => fetched.elapsed() > CACHE_TTL,
        }
    }
}

pub struct OembedFetcher;

impl OembedFetcher {
    pub fn new() -> Self {
        Self
    }

    /// Ensure providers are loaded and fresh
    fn ensure_providers() -> anyhow::Result<()> {
        let needs_refresh = {
            let cache = PROVIDER_CACHE.read().unwrap();
            cache.is_stale()
        };

        if needs_refresh {
            log::debug!("Provider cache is stale, refreshing...");
            match Self::fetch_providers() {
                Ok(providers) => {
                    let mut cache = PROVIDER_CACHE.write().unwrap();
                    cache.providers = providers;
                    cache.fetched_at = Some(Instant::now());
                    log::info!("Successfully fetched {} oEmbed providers", cache.providers.len());
                }
                Err(e) => {
                    log::warn!("Failed to fetch providers from {}: {}", PROVIDERS_URL, e);
                    let mut cache = PROVIDER_CACHE.write().unwrap();
                    if cache.providers.is_empty() {
                        log::info!("Using fallback provider list");
                        cache.providers = Self::fallback_providers();
                        cache.fetched_at = Some(Instant::now());
                    } else {
                        log::info!("Keeping existing cached providers");
                    }
                }
            }
        }

        Ok(())
    }

    /// Fetch provider list from oembed.com
    fn fetch_providers() -> anyhow::Result<Vec<Provider>> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let response = client.get(PROVIDERS_URL).send()?;
        let providers: Vec<Provider> = response.json()?;
        Ok(providers)
    }

    /// Hardcoded fallback for top providers
    fn fallback_providers() -> Vec<Provider> {
        serde_json::from_value(serde_json::json!([
            {
                "provider_name": "YouTube",
                "provider_url": "https://www.youtube.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "https://*.youtube.com/watch*",
                            "https://*.youtube.com/v/*",
                            "https://youtu.be/*",
                            "https://*.youtube.com/playlist?list=*",
                            "https://youtube.com/playlist?list=*",
                            "https://*.youtube.com/shorts/*"
                        ],
                        "url": "https://www.youtube.com/oembed"
                    }
                ]
            },
            {
                "provider_name": "Vimeo",
                "provider_url": "https://vimeo.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "https://vimeo.com/*",
                            "https://vimeo.com/album/*/video/*",
                            "https://vimeo.com/channels/*/*",
                            "https://vimeo.com/groups/*/videos/*",
                            "https://vimeo.com/ondemand/*/*",
                            "https://player.vimeo.com/video/*"
                        ],
                        "url": "https://vimeo.com/api/oembed.json"
                    }
                ]
            },
            {
                "provider_name": "Twitter",
                "provider_url": "https://www.twitter.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "https://twitter.com/*/status/*",
                            "https://*.twitter.com/*/status/*",
                            "https://twitter.com/*/moments/*",
                            "https://*.twitter.com/*/moments/*"
                        ],
                        "url": "https://publish.twitter.com/oembed"
                    }
                ]
            },
            {
                "provider_name": "Instagram",
                "provider_url": "https://instagram.com",
                "endpoints": [
                    {
                        "schemes": [
                            "http://instagram.com/*/p/*,",
                            "http://www.instagram.com/*/p/*,",
                            "https://instagram.com/*/p/*,",
                            "https://www.instagram.com/*/p/*,",
                            "http://instagram.com/p/*",
                            "http://instagr.am/p/*",
                            "http://www.instagram.com/p/*",
                            "http://www.instagr.am/p/*",
                            "https://instagram.com/p/*",
                            "https://instagr.am/p/*",
                            "https://www.instagram.com/p/*",
                            "https://www.instagr.am/p/*",
                            "http://instagram.com/tv/*",
                            "http://instagr.am/tv/*",
                            "http://www.instagram.com/tv/*",
                            "http://www.instagr.am/tv/*",
                            "https://instagram.com/tv/*",
                            "https://instagr.am/tv/*",
                            "https://www.instagram.com/tv/*",
                            "https://www.instagr.am/tv/*",
                            "http://www.instagram.com/reel/*",
                            "https://www.instagram.com/reel/*",
                            "http://instagram.com/reel/*",
                            "https://instagram.com/reel/*",
                            "http://instagr.am/reel/*",
                            "https://instagr.am/reel/*"
                        ],
                        "url": "https://graph.facebook.com/v16.0/instagram_oembed"
                    }
                ]
            },
            {
                "provider_name": "Spotify",
                "provider_url": "https://spotify.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "https://open.spotify.com/*",
                            "https://play.spotify.com/*"
                        ],
                        "url": "https://open.spotify.com/oembed"
                    }
                ]
            },
            {
                "provider_name": "SoundCloud",
                "provider_url": "https://soundcloud.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "https://soundcloud.com/*",
                            "https://soundcloud.app.goo.gl/*",
                            "https://on.soundcloud.com/*"
                        ],
                        "url": "https://soundcloud.com/oembed"
                    }
                ]
            },
            {
                "provider_name": "TikTok",
                "provider_url": "http://www.tiktok.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "https://www.tiktok.com/*",
                            "https://www.tiktok.com/*/video/*"
                        ],
                        "url": "https://www.tiktok.com/oembed"
                    }
                ]
            },
            {
                "provider_name": "Flickr",
                "provider_url": "https://www.flickr.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "http://*.flickr.com/photos/*",
                            "http://flic.kr/p/*",
                            "https://*.flickr.com/photos/*",
                            "https://flic.kr/p/*",
                            "https://*.*.flickr.com/*/*",
                            "http://*.*.flickr.com/*/*"
                        ],
                        "url": "https://www.flickr.com/services/oembed/"
                    }
                ]
            },
            {
                "provider_name": "Reddit",
                "provider_url": "https://reddit.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "https://reddit.com/r/*/comments/*/*",
                            "https://www.reddit.com/r/*/comments/*/*"
                        ],
                        "url": "https://www.reddit.com/oembed"
                    }
                ]
            },
            {
                "provider_name": "Dailymotion",
                "provider_url": "https://www.dailymotion.com",
                "endpoints": [
                    {
                        "schemes": [
                            "https://www.dailymotion.com/video/*"
                        ],
                        "url": "https://www.dailymotion.com/services/oembed"
                    }
                ]
            },
            {
                "provider_name": "SlideShare",
                "provider_url": "https://www.slideshare.net/",
                "endpoints": [
                    {
                        "schemes": [
                            "https://www.slideshare.net/*/*",
                            "http://www.slideshare.net/*/*",
                            "https://de.slideshare.net/*/*",
                            "http://de.slideshare.net/*/*",
                            "https://es.slideshare.net/*/*",
                            "http://es.slideshare.net/*/*",
                            "https://fr.slideshare.net/*/*",
                            "http://fr.slideshare.net/*/*",
                            "https://pt.slideshare.net/*/*",
                            "http://pt.slideshare.net/*/*"
                        ],
                        "url": "https://www.slideshare.net/api/oembed/2"
                    }
                ]
            },
            {
                "provider_name": "Tumblr",
                "provider_url": "https://www.tumblr.com",
                "endpoints": [
                    {
                        "schemes": [
                            "https://*.tumblr.com/post/*",
                            "http://*.tumblr.com/post/*"
                        ],
                        "url": "https://www.tumblr.com/oembed/1.0"
                    }
                ]
            },
            {
                "provider_name": "Kickstarter",
                "provider_url": "http://www.kickstarter.com",
                "endpoints": [
                    {
                        "schemes": [
                            "https://www.kickstarter.com/projects/*"
                        ],
                        "url": "https://www.kickstarter.com/services/oembed"
                    }
                ]
            },
            {
                "provider_name": "Imgur",
                "provider_url": "https://imgur.com/",
                "endpoints": [
                    {
                        "schemes": [
                            "http://imgur.com/*",
                            "http://imgur.com/*/embed",
                            "http://imgur.com/gallery/*",
                            "http://imgur.com/a/*",
                            "https://imgur.com/*",
                            "https://imgur.com/*/embed",
                            "https://imgur.com/gallery/*",
                            "https://imgur.com/a/*"
                        ],
                        "url": "https://api.imgur.com/oembed"
                    }
                ]
            },
            {
                "provider_name": "CodePen",
                "provider_url": "https://codepen.io",
                "endpoints": [
                    {
                        "schemes": [
                            "http://codepen.io/*",
                            "https://codepen.io/*"
                        ],
                        "url": "https://codepen.io/api/oembed"
                    }
                ]
            }
        ]))
        .unwrap()
    }

    /// Check if URL matches any provider scheme
    fn find_endpoint(url: &str) -> Option<String> {
        let cache = PROVIDER_CACHE.read().unwrap();

        for provider in &cache.providers {
            for endpoint in &provider.endpoints {
                for scheme in &endpoint.schemes {
                    if Self::matches_scheme(url, scheme) {
                        log::debug!("URL {} matched provider {} with scheme {}", url, provider.provider_name, scheme);
                        return Some(endpoint.url.clone());
                    }
                }
            }
        }

        None
    }

    /// Simple glob-style matching: convert * to .* and match
    fn matches_scheme(url: &str, scheme: &str) -> bool {
        let pattern = scheme
            .replace(".", "\\.")
            .replace("*", ".*")
            .replace("?", "\\?");

        let regex = match regex::Regex::new(&format!("^{}$", pattern)) {
            Ok(r) => r,
            Err(_) => return false,
        };

        regex.is_match(url)
    }

    /// Fetch oEmbed data from endpoint
    fn fetch_oembed(endpoint_url: &str, target_url: &str, _scrape_config: Option<&ScrapeConfig>) -> anyhow::Result<OembedResponse> {
        let client = reqwest::blocking::Client::builder()
            .timeout(OEMBED_TIMEOUT)
            .build()?;

        let encoded_url = reqwest::Url::parse(target_url)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| target_url.to_string());
        let full_url = format!("{}?url={}&format=json", endpoint_url, encoded_url);
        log::debug!("Fetching oEmbed from: {}", full_url);

        let response = client.get(&full_url).send()?;

        if !response.status().is_success() {
            anyhow::bail!("oEmbed endpoint returned status {}", response.status());
        }

        let oembed: OembedResponse = response.json()?;
        Ok(oembed)
    }

    /// Convert oEmbed response to Metadata
    fn oembed_to_metadata(oembed: OembedResponse, scrape_config: Option<&ScrapeConfig>) -> Metadata {
        let title = oembed.title.or_else(|| {
            match (&oembed.author_name, &oembed.provider_name) {
                (Some(author), Some(provider)) => Some(format!("{} - {}", author, provider)),
                (Some(author), None) => Some(author.clone()),
                (None, Some(provider)) => Some(provider.clone()),
                (None, None) => None,
            }
        });

        let image = oembed.thumbnail_url.as_ref().and_then(|img_url| {
            crate::metadata::fetchers::fetch_bytes(img_url, scrape_config)
        });

        Metadata {
            title,
            description: oembed.description,
            image_url: oembed.thumbnail_url,
            image,
            ..Default::default()
        }
    }
}

impl MetadataFetcher for OembedFetcher {
    fn fetch(&self, url: &str, scrape_config: Option<&ScrapeConfig>) -> anyhow::Result<FetchOutcome> {
        // Ensure providers are loaded
        Self::ensure_providers()?;

        // Find matching endpoint
        let endpoint_url = match Self::find_endpoint(url) {
            Some(ep) => ep,
            None => {
                log::debug!("No oEmbed provider matched URL: {}", url);
                return Ok(FetchOutcome::Skip("no matching oEmbed provider".into()));
            }
        };

        // Fetch oEmbed data
        match Self::fetch_oembed(&endpoint_url, url, scrape_config) {
            Ok(oembed) => {
                let metadata = Self::oembed_to_metadata(oembed, scrape_config);

                // Only return Some if we got useful data
                if metadata.has_any_data() {
                    Ok(FetchOutcome::Data(metadata))
                } else {
                    Ok(FetchOutcome::Skip("oEmbed response empty".into()))
                }
            }
            Err(e) => {
                log::warn!("oEmbed fetch failed for {}: {}", url, e);
                Err(e)
            }
        }
    }

    fn name(&self) -> &'static str {
        "oEmbed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_matching_youtube() {
        // Ensure fallback providers are loaded
        let mut cache = PROVIDER_CACHE.write().unwrap();
        cache.providers = OembedFetcher::fallback_providers();
        cache.fetched_at = Some(Instant::now());
        drop(cache);

        let youtube_urls = vec![
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://www.youtube.com/v/dQw4w9WgXcQ",
            "https://www.youtube.com/shorts/abc123",
        ];

        for url in youtube_urls {
            let endpoint = OembedFetcher::find_endpoint(url);
            assert!(endpoint.is_some(), "Failed to match YouTube URL: {}", url);
            assert_eq!(endpoint.unwrap(), "https://www.youtube.com/oembed");
        }
    }

    #[test]
    fn test_url_matching_vimeo() {
        let mut cache = PROVIDER_CACHE.write().unwrap();
        cache.providers = OembedFetcher::fallback_providers();
        cache.fetched_at = Some(Instant::now());
        drop(cache);

        let vimeo_urls = vec![
            "https://vimeo.com/123456789",
            "https://vimeo.com/channels/staffpicks/123456789",
            "https://player.vimeo.com/video/123456789",
        ];

        for url in vimeo_urls {
            let endpoint = OembedFetcher::find_endpoint(url);
            assert!(endpoint.is_some(), "Failed to match Vimeo URL: {}", url);
            assert_eq!(endpoint.unwrap(), "https://vimeo.com/api/oembed.json");
        }
    }

    #[test]
    fn test_non_matching_url() {
        let mut cache = PROVIDER_CACHE.write().unwrap();
        cache.providers = OembedFetcher::fallback_providers();
        cache.fetched_at = Some(Instant::now());
        drop(cache);

        let non_oembed_urls = vec![
            "https://example.com/page",
            "https://news.ycombinator.com/",
            "https://github.com/user/repo",
        ];

        for url in non_oembed_urls {
            let endpoint = OembedFetcher::find_endpoint(url);
            assert!(endpoint.is_none(), "Incorrectly matched non-oEmbed URL: {}", url);
        }
    }

    #[test]
    fn test_oembed_json_parsing() {
        let json = serde_json::json!({
            "type": "video",
            "title": "Test Video",
            "author_name": "Test Author",
            "provider_name": "YouTube",
            "thumbnail_url": "https://example.com/thumb.jpg",
            "description": "A test video"
        });

        let oembed: OembedResponse = serde_json::from_value(json).unwrap();
        assert_eq!(oembed.title, Some("Test Video".to_string()));
        assert_eq!(oembed.author_name, Some("Test Author".to_string()));
        assert_eq!(oembed.provider_name, Some("YouTube".to_string()));
        assert_eq!(oembed.thumbnail_url, Some("https://example.com/thumb.jpg".to_string()));
        assert_eq!(oembed.description, Some("A test video".to_string()));
    }

    #[test]
    fn test_fallback_providers_populated() {
        let providers = OembedFetcher::fallback_providers();
        assert!(!providers.is_empty(), "Fallback providers should not be empty");

        // Check for key providers
        let provider_names: Vec<&str> = providers.iter().map(|p| p.provider_name.as_str()).collect();
        assert!(provider_names.contains(&"YouTube"));
        assert!(provider_names.contains(&"Vimeo"));
        assert!(provider_names.contains(&"Twitter"));
        assert!(provider_names.contains(&"Spotify"));
    }

    #[test]
    fn test_partial_results_title_only() {
        let oembed = OembedResponse {
            oembed_type: Some("video".to_string()),
            title: Some("Test Video".to_string()),
            author_name: None,
            provider_name: None,
            thumbnail_url: None,
            description: None,
        };

        let metadata = OembedFetcher::oembed_to_metadata(oembed, None);
        assert_eq!(metadata.title, Some("Test Video".to_string()));
        assert!(metadata.image_url.is_none());
        assert!(metadata.has_any_data());
    }

    #[test]
    fn test_partial_results_image_only() {
        let oembed = OembedResponse {
            oembed_type: Some("photo".to_string()),
            title: None,
            author_name: None,
            provider_name: None,
            thumbnail_url: Some("https://example.com/image.jpg".to_string()),
            description: None,
        };

        let metadata = OembedFetcher::oembed_to_metadata(oembed, None);
        assert!(metadata.title.is_none());
        assert_eq!(metadata.image_url, Some("https://example.com/image.jpg".to_string()));
        assert!(metadata.has_any_data());
    }

    #[test]
    fn test_title_construction_from_author_and_provider() {
        let oembed = OembedResponse {
            oembed_type: Some("video".to_string()),
            title: None,
            author_name: Some("John Doe".to_string()),
            provider_name: Some("YouTube".to_string()),
            thumbnail_url: None,
            description: None,
        };

        let metadata = OembedFetcher::oembed_to_metadata(oembed, None);
        assert_eq!(metadata.title, Some("John Doe - YouTube".to_string()));
    }

    #[test]
    fn test_scheme_matching() {
        assert!(OembedFetcher::matches_scheme(
            "https://www.youtube.com/watch?v=abc",
            "https://*.youtube.com/watch*"
        ));

        assert!(OembedFetcher::matches_scheme(
            "https://youtu.be/abc123",
            "https://youtu.be/*"
        ));

        assert!(!OembedFetcher::matches_scheme(
            "https://example.com/video",
            "https://youtube.com/*"
        ));
    }
}
