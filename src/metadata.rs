use crate::scrape;
use anyhow::bail;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub fn fetch_meta(url: &str, opts: MetaOptions) -> anyhow::Result<Metadata> {
    log::debug!("trying plain request");

    let meta = match scrape::fetch_page_with_reqwest(url) {
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
            } else {
                if !opts.no_headless {
                    log::debug!("cover not found, taking screencapture");
                    if let Some(chrome_result) = scrape::fetch_page_with_chrome(url) {
                        log::debug!("screencapture is taken");

                        // now that we've captured the page with browser, we might as well
                        // update the meta data, since the capture is generally more accurate.
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
                            if let Some(image_url) = m.image_url {
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
                if let Some((status, bytes)) = scrape::reqwest_with_retries(dbg!(icon_url)) {
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

                if let Some(chrome_result) = scrape::fetch_page_with_chrome(url) {
                    let mut meta = scrape::get_data_from_page(chrome_result.html, url);

                    // TODO: UNCOMMENT
                    if let Some(ref image_url) = meta.image_url {
                        if meta.image.is_none() {
                            log::debug!("fetching cover");

                            if let Some((status, bytes)) = scrape::reqwest_with_retries(image_url) {
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

                        if let Some((status, bytes)) = scrape::reqwest_with_retries(icon_url) {
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
    };

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
