use crate::scrape;
use anyhow::bail;
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

fn get_youtube_image_url(url: &str) -> Option<String> {
    let youtube_regex = regex::Regex::new(
            r"(?:https?://)?(?:www\.)?(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/shorts/)([a-zA-Z0-9_-]{11})"
        ).unwrap();

    if let Some(captures) = youtube_regex.captures(url) {
        if let Some(video_id) = captures.get(1) {
            let video_id = video_id.as_str();
            return Some(format!(
                "https://img.youtube.com/vi/{video_id}/maxresdefault.jpg"
            ));
        }
    }
    None
}

pub fn fetch_meta(url: &str, opts: MetaOptions) -> anyhow::Result<Metadata> {
    log::debug!("trying plain request");

    let mut meta = match scrape::fetch_page_with_reqwest(url) {
        Some(reqwest_result) => {
            log::debug!("plain request successful");

            let mut meta = scrape::get_data_from_page(reqwest_result.html, url);

            if let Some(ref image_url) = meta.image_url {
                log::debug!("fetching cover");

                if let Some((status, bytes)) = scrape::reqwest_with_retries(image_url) {
                    if status.is_success() {
                        log::debug!("cover is fetched");
                        meta.image = Some(bytes);
                    }
                }
            } else if !opts.no_headless {
                log::debug!("cover not found, taking screencapture");

                #[cfg(feature = "headless")]
                if let Some(chrome_result) = scrape::headless::fetch_page_with_chrome(url) {
                    log::debug!("screencapture is taken");

                    if !chrome_result.html.contains("Sorry, you have been blocked")
                        && !chrome_result
                            .html
                            .contains("Verify you are human by completing the action below")
                    {
                        // update the entire meta data, since the capture is generally more accurate.
                        {
                            let m = scrape::get_data_from_page(chrome_result.html, url);
                            if let Some(title) = m.title {
                                meta.title.replace(title);
                            }
                            if let Some(description) = m.description {
                                meta.description.replace(description);
                            }
                            if let Some(icon_url) = m.icon_url {
                                meta.icon_url.replace(icon_url);
                            }
                            if let Some(image_url) =
                                get_youtube_image_url(url).map(Some).unwrap_or(m.image_url)
                            {
                                meta.image_url.replace(image_url.clone());
                                if let Some((status, bytes)) =
                                    scrape::reqwest_with_retries(&image_url)
                                {
                                    if status.is_success() {
                                        log::debug!("cover is fetched");
                                        meta.image = Some(bytes);
                                    }
                                }
                            }
                        }

                        if meta.image.is_none() {
                            meta.image.replace(chrome_result.screenshot);
                        }
                    }
                }
            }

            if let Some(ref icon_url) = meta.icon_url {
                log::debug!("fetching icon");
                if let Some((status, bytes)) = scrape::reqwest_with_retries(icon_url) {
                    if status.is_success() {
                        log::debug!("icon is fetched");

                        meta.icon = Some(bytes.to_vec());
                    }
                }
            }

            Some(meta)
        }
        None => {
            if !opts.no_headless {
                log::debug!("plain request failed. trying chromium.");

                #[cfg(feature = "headless")]
                {
                    if let Some(chrome_result) = scrape::headless::fetch_page_with_chrome(url) {
                        if !chrome_result.html.contains("Sorry, you have been blocked")
                            && !chrome_result
                                .html
                                .contains("Verify you are human by completing the action below")
                        {
                            let mut meta = scrape::get_data_from_page(chrome_result.html, url);

                            // TODO: UNCOMMENT
                            if let Some(ref image_url) = get_youtube_image_url(url)
                                .map(Some)
                                .unwrap_or(meta.image_url.clone())
                            {
                                if meta.image.is_none() {
                                    log::debug!("fetching cover");

                                    if let Some((status, bytes)) =
                                        scrape::reqwest_with_retries(image_url)
                                    {
                                        if status.is_success() {
                                            log::debug!("cover is fetched");

                                            meta.image = Some(bytes);
                                        }
                                    }
                                }
                            }

                            if meta.image.is_none() {
                                meta.image = Some(chrome_result.screenshot);
                            }

                            if let Some(ref icon_url) = meta.icon_url {
                                log::debug!("fetching icon");

                                if let Some((status, bytes)) =
                                    scrape::reqwest_with_retries(icon_url)
                                {
                                    if status.is_success() {
                                        log::debug!("icon is fetched");

                                        meta.icon = Some(bytes.to_vec());
                                    }
                                }
                            }

                            Some(meta)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }

                #[cfg(not(feature = "headless"))]
                None
            } else {
                None
            }
        }
    };

    if match meta {
        None => true,
        Some(ref m) => {
            m.title.is_none()
                && m.description.is_none()
                && m.icon_url.is_none()
                && m.image_url.is_none()
                && m.keywords.is_none()
                && m.canonical_url.is_none()
        }
    } {
        scrape::get_data_from_ddg(url).map(|mut m| {
            if let Some(ref icon_url) = m.icon_url {
                log::debug!("fetching icon");

                if let Some((status, bytes)) = scrape::reqwest_with_retries(icon_url) {
                    if status.is_success() {
                        log::debug!("icon is fetched");

                        m.icon = Some(bytes.to_vec());
                    }
                }
            }
            meta = Some(m);
        });
    }

    match meta {
        Some(mut meta) => {
            // try to get favicon from duckduckgo
            if meta.icon.is_none() {
                log::debug!("get favicon from duckduckgo");
                let url_parsed = reqwest::Url::parse(url).unwrap();
                let host = url_parsed.host_str();

                if let Some(host) = host {
                    let icon_url =
                        format!("https://external-content.duckduckgo.com/ip3/{host}.ico");

                    if let Some((status, bytes)) = scrape::reqwest_with_retries(&icon_url) {
                        if status.is_success() {
                            log::debug!("icon is fetched");

                            meta.icon = Some(bytes.to_vec());
                            meta.icon_url = Some(icon_url);
                        }
                    }
                }
            }

            Ok(meta)
        }
        None => bail!("couldnt't retrieve metadata"),
    }
}
