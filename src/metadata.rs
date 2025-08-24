use crate::scrape;
use anyhow::{bail, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::env;

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

/// Compile YouTube regex once
static YOUTUBE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?:https?://)?(?:www\.)?(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/shorts/)([A-Za-z0-9_-]{11})",
    )
    .expect("Failed to compile YouTube regex")
});

fn get_youtube_image_url(url: &str) -> Option<String> {
    YOUTUBE_REGEX
        .captures(url)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_owned()))
        .map(|video_id| format!("https://img.youtube.com/vi/{}/maxresdefault.jpg", video_id))
}

/// Helper to fetch bytes from a URL via scrape::reqwest_with_retries
fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    if let Some((status, bytes)) = scrape::reqwest_with_retries(url) {
        if status.is_success() {
            return Some(bytes);
        }
    }
    None
}

/// Given a Metadata and an image_url field name, try to fetch bytes and set the `image` field if success.
fn try_fetch_image(meta: &mut Metadata) {
    if let Some(ref img_url) = meta.image_url {
        log::debug!("fetching image from {}", img_url);
        if let Some(bytes) = fetch_bytes(img_url) {
            log::debug!("image fetched");
            meta.image = Some(bytes);
        }
    }
}

/// Given a Metadata and an icon_url, try to fetch bytes and set the `icon` field if success.
fn try_fetch_icon(meta: &mut Metadata) {
    if let Some(ref icon_url) = meta.icon_url {
        log::debug!("fetching icon from {}", icon_url);
        if let Some(bytes) = fetch_bytes(icon_url) {
            log::debug!("icon fetched");
            meta.icon = Some(bytes);
        }
    }
}

/// Attempt Peekalink-based fetch. Returns `Ok(Some(meta))` if successful, `Ok(None)` if cannot use Peekalink or missing critical fields, or Err if calling peekalink failed critically.
/// Note: `peekalink::peekalink` is assumed to return Option<some struct with fields>.
fn fetch_via_peekalink(url: &str) -> Result<Option<Metadata>> {
    let api_key = match env::var("PEEKALINK_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            log::warn!("PEEKALINK_API_KEY is missing");
            return Ok(None)
        }
    };

    let peek_result = crate::peekalink::peekalink(url, &api_key);
    log::info!("peekalink result: {:#?}", peek_result);

    if let Some(m) = peek_result {
        // require at least title and image_url
        if m.title.is_some() && m.image_url.is_some() {
            let mut meta = Metadata {
                title: m.title,
                description: m.description,
                canonical_url: m.canonical_url,
                icon_url: m.icon_url,
                image_url: m.image_url.clone(),
                keywords: None,
                dump: None,
                image: None,
                icon: None,
            };
            // fetch image and icon
            if let Some(img_url) = meta.image_url.clone() {
                if let Some(bytes) = fetch_bytes(&img_url) {
                    meta.image = Some(bytes);
                }
            }
            if let Some(icon_url) = meta.icon_url.clone() {
                if let Some(bytes) = fetch_bytes(&icon_url) {
                    meta.icon = Some(bytes);
                }
            }
            return Ok(Some(meta));
        }
    }
    Ok(None)
}

/// Core “plain” fetch: try reqwest fetch_page, parse metadata, attempt image/icon, and optional headless capture for missing parts.
fn fetch_plain_or_headless(url: &str, opts: &MetaOptions) -> Option<Metadata> {
    if let Some(reqwest_result) = scrape::fetch_page_with_reqwest(url) {
        log::debug!("plain request successful");
        let mut meta = scrape::get_data_from_page(reqwest_result.html.clone(), url);

        // Try fetching image if present
        if meta.image.is_none() {
            try_fetch_image(&mut meta);
        }

        // If still no image and headless allowed, take screenshot & re-parse page if useful
        if meta.image.is_none() && !opts.no_headless {
            log::debug!("cover not found, attempting headless capture");
            #[cfg(feature = "headless")]
            if let Some(chrome_res) = scrape::headless::fetch_page_with_chrome(url) {
                let blocked = chrome_res.html.contains("Sorry, you have been blocked")
                    || chrome_res
                        .html
                        .contains("Verify you are human by completing the action below");
                if !blocked {
                    // Re-parse to get possibly better metadata
                    let new_meta = scrape::get_data_from_page(chrome_res.html.clone(), url);
                    // Update fields if present
                    if let Some(t) = new_meta.title {
                        meta.title = Some(t);
                    }
                    if let Some(d) = new_meta.description {
                        meta.description = Some(d);
                    }
                    if let Some(icon_u) = new_meta.icon_url {
                        meta.icon_url = Some(icon_u);
                    }
                    // For image_url: prefer YouTube thumbnail or new_meta.image_url
                    let youtube_img = get_youtube_image_url(url);
                    let chosen_img_url = youtube_img.or(new_meta.image_url.clone());
                    if let Some(img_url) = chosen_img_url {
                        meta.image_url = Some(img_url.clone());
                        if let Some(bytes) = fetch_bytes(&img_url) {
                            log::debug!("image fetched from headless-detected URL");
                            meta.image = Some(bytes);
                        }
                    }
                    // If still no image, use screenshot
                    if meta.image.is_none() {
                        meta.image = Some(chrome_res.screenshot);
                    }
                }
            }
        }

        // Try fetching icon if present or after headless
        if meta.icon.is_none() {
            try_fetch_icon(&mut meta);
        }
        return Some(meta);
    } else if !opts.no_headless {
        // Plain reqwest failed: try headless if allowed
        log::debug!("plain request failed, trying headless");
        #[cfg(feature = "headless")]
        {
            if let Some(chrome_res) = scrape::headless::fetch_page_with_chrome(url) {
                let blocked = chrome_res.html.contains("Sorry, you have been blocked")
                    || chrome_res
                        .html
                        .contains("Verify you are human by completing the action below");
                if !blocked {
                    let mut meta = scrape::get_data_from_page(chrome_res.html.clone(), url);
                    // Try YouTube thumbnail first if no image
                    if meta.image.is_none() {
                        if let Some(y_img) = get_youtube_image_url(url) {
                            if let Some(bytes) = fetch_bytes(&y_img) {
                                meta.image = Some(bytes);
                                meta.image_url = Some(y_img.clone());
                            }
                        }
                    }
                    if meta.image.is_none() {
                        meta.image = Some(chrome_res.screenshot);
                    }
                    // Try icon
                    if meta.icon.is_none() {
                        try_fetch_icon(&mut meta);
                    }
                    return Some(meta);
                }
            }
        }
        // If either not feature or headless failed
    }
    None
}

/// If initial meta is empty (all fields None), try DuckDuckGo fallback.
fn fetch_fallback_ddg(url: &str) -> Option<Metadata> {
    scrape::get_data_from_ddg(url).map(|mut m| {
        if m.icon.is_none() {
            if let Some(ref icon_url) = m.icon_url {
                if let Some(bytes) = fetch_bytes(icon_url) {
                    m.icon = Some(bytes);
                }
            }
        }
        m
    })
}

/// After obtaining some Metadata, if icon is still missing, try DuckDuckGo favicon endpoint.
fn try_fetch_ddg_favicon(meta: &mut Metadata, url: &str) {
    if meta.icon.is_none() {
        log::debug!("attempting DuckDuckGo favicon");
        if let Ok(parsed) = reqwest::Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                let icon_url = format!("https://external-content.duckduckgo.com/ip3/{host}.ico");
                if let Some(bytes) = fetch_bytes(&icon_url) {
                    log::debug!("favicon fetched from DuckDuckGo");
                    meta.icon = Some(bytes);
                    meta.icon_url = Some(icon_url);
                }
            }
        }
    }
}

pub fn fetch_meta(url: &str, opts: MetaOptions) -> Result<Metadata> {
    // 1. Try Peekalink if API key present
    if let Some(mut meta) = fetch_via_peekalink(url)? {
        try_fetch_ddg_favicon(&mut meta, url);
        return Ok(meta);
    }

    // 2. Try plain/headless path
    let mut meta_opt = fetch_plain_or_headless(url, &opts);

    // 3. If obtained meta is empty (no useful fields), try DDG fallback
    let is_empty = |m: &Metadata| {
        m.title.is_none()
            && m.description.is_none()
            && m.icon_url.is_none()
            && m.image_url.is_none()
            && m.keywords.is_none()
            && m.canonical_url.is_none()
    };
    if meta_opt.as_ref().map_or(true, |m| is_empty(m)) {
        if let Some(fb_meta) = fetch_fallback_ddg(url) {
            meta_opt = Some(fb_meta);
        }
    }

    // 4. If still none, bail
    let mut meta = meta_opt.ok_or_else(|| anyhow::anyhow!("Couldn't retrieve metadata"))?;

    // 5. Final attempt: DuckDuckGo favicon if icon missing
    try_fetch_ddg_favicon(&mut meta, url);

    Ok(meta)
}
