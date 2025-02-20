#[cfg(feature = "headless")]
pub mod headless;

use reqwest::StatusCode;
use std::{cmp::Ordering, error::Error, thread::sleep, time::Duration};

use crate::metadata::Metadata;
const USER_AGENT_DEFAULT: &'static str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0";

fn get_error(error: &reqwest::Error) -> String {
    match error.source() {
        Some(e) => match e.source() {
            Some(e) => e.to_string(),
            None => e.to_string(),
        },
        None => error.to_string(),
    }
}

pub fn reqwest_with_retries(url: &str) -> Option<(StatusCode, Vec<u8>)> {
    let opt_proxy = std::env::var("OPT_PROXY").unwrap_or_default().to_string();

    let mut r = 0;

    let url_parsed = reqwest::Url::parse(url).unwrap();
    let host = url_parsed.host_str().unwrap_or_default();
    let path = url_parsed.path();
    let iden = format!("{host}{path}");

    let mut force_proxy = false;
    loop {
        if r >= 5 {
            return None;
        }

        if r > 0 {
            log::debug!("{iden}: retrying");
        }

        r += 1;

        let mut client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT_DEFAULT)
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(10));

        if force_proxy && !opt_proxy.is_empty() {
            log::debug!("{iden}: using proxy {opt_proxy:#?}");
            client = client.proxy(reqwest::Proxy::all(&opt_proxy).unwrap());
        }

        let client = client.build().unwrap();

        log::debug!("{iden}: requesting");

        let resp = match client.get(url).send() {
            Ok(r) => r,
            Err(err) => {
                force_proxy = true;
                log::error!("{iden}: {err}: {:#?}", get_error(&err));
                continue;
            }
        };

        let status = resp.status();

        if !status.is_success() {
            log::debug!("{iden}: {:?}", resp.status().to_string());
        }

        if status == StatusCode::OK {
            // we might get OK, but no text response.
            // resp.text().unwrap();
            let bytes = match resp.bytes() {
                Ok(b) => b,
                Err(err) => {
                    log::debug!("{iden}: {}", err.is_timeout());
                    force_proxy = true;
                    continue;
                }
            };

            return Some((status, bytes.into()));
        }

        if status == StatusCode::TOO_MANY_REQUESTS {
            sleep(Duration::from_secs(r * 4));
        }

        if status.is_client_error() {
            // no need to try again, it's over...
            if force_proxy {
                return None;
            }

            force_proxy = true;
        }
    }
}

pub struct ReqwestResult {
    pub html: String,
}
pub fn fetch_page_with_reqwest(url: &str) -> Option<ReqwestResult> {
    reqwest_with_retries(url).map(|(_status, bytes)| ReqwestResult {
        html: String::from_utf8_lossy(&bytes).to_string(),
    })
}

pub fn get_data_from_ddg(url: &str) -> Option<Metadata> {
    let ddg_url = format!("https://lite.duckduckgo.com/lite/?q={url}");
    match reqwest_with_retries(&ddg_url) {
        Some((status, bytes)) => {
            if !status.is_success() {
                return None;
            }

            get_data_from_ddg_html(String::from_utf8_lossy(&bytes).to_string(), &ddg_url)
        }
        None => None,
    }
}

pub fn get_data_from_ddg_html(resp_text: String, url: &str) -> Option<Metadata> {
    let document = scraper::Html::parse_document(&resp_text);
    let body_selector = scraper::Selector::parse("body").unwrap();

    let title_selector = scraper::Selector::parse(".result-link").unwrap();
    let description_selector = scraper::Selector::parse(".result-snippet").unwrap();

    let mut description = None;
    let mut title = None;
    let mut icon_url = None;

    let body = document.select(&body_selector).next().unwrap();
    body.select(&title_selector).next().map(|heading_el| {
        heading_el.text().next().map(|title_text| {
            title = Some(title_text.to_string().trim().to_string());
        })
    });
    body.select(&description_selector).next().map(|desc_el| {
        desc_el.text().next().map(|desc_text| {
            description = Some(desc_text.to_string().trim().to_string());
        })
    });

    if icon_url.is_none() {
        let url_parsed = reqwest::Url::parse(url).unwrap();
        let host = url_parsed.host_str();

        if let Some(host) = host {
            icon_url = Some(format!(
                "https://external-content.duckduckgo.com/ip3/{host}.ico"
            ));
        }
    }

    if title.is_none() {
        return None;
    }

    Some(Metadata {
        title,
        description,
        keywords: None,
        canonical_url: None,
        image_url: None,
        icon_url,
        image: None,
        icon: None,
        dump: None,
    })
}

pub fn get_data_from_page(resp_text: String, url: &str) -> Metadata {
    let document = scraper::Html::parse_document(&resp_text);
    let head_selector = scraper::Selector::parse("head").unwrap();
    let meta_selector = scraper::Selector::parse("meta").unwrap();
    let title_selector = scraper::Selector::parse("title").unwrap();
    let link_selector = scraper::Selector::parse("link").unwrap();

    let mut description = None;
    let mut keywords = None;
    let mut title = None;
    let mut image_url = None;

    #[allow(unused_assignments)]
    let mut icon_url = None;
    let mut canonical_url = None;

    let head = document.select(&head_selector).next().unwrap();
    for element in head.select(&meta_selector) {
        let meta_prop = element.attr("property").unwrap_or_default();

        let meta_key = element.attr("name").or(Some(meta_prop)).unwrap_or_default();
        let meta_value = element.attr("content").unwrap_or_default();

        // parse description
        if description.is_none()
            && [
                "Description",
                "description",
                "og:description",
                "og:Description",
            ]
            .into_iter()
            .find(|name| *name == meta_key)
            .is_some()
        {
            description = Some(meta_value.to_string());
        }

        // parse keywords
        if keywords.is_none()
            && ["Keywords", "keywords"]
                .into_iter()
                .find(|name| *name == meta_key)
                .is_some()
        {
            keywords = Some(meta_value.to_string());
        }

        // parse og image
        if image_url.is_none()
            && ["og:image", "twitter:image"]
                .into_iter()
                .find(|name| *name == meta_key)
                .is_some()
        {
            image_url = Some(meta_value.to_string());
        }

        // parse canonical url
        if canonical_url.is_none() && meta_key == "og:url" {
            canonical_url = Some(meta_value.to_string());
        }
    }

    // TODO: parse favicon urls. keep in mind, href could be relative.
    //
    // let url_parsed = reqwest::Url::parse(url).unwrap();
    // let host = url_parsed.host_str().unwrap_or_default();
    let mut icons = Vec::new();
    for element in head.select(&link_selector) {
        let link_href = element.attr("href").unwrap_or_default();
        let link_rel = element.attr("rel").unwrap_or_default();
        let link_type = element.attr("type").unwrap_or_default();
        let link_sizes = element.attr("sizes").unwrap_or_default();

        if link_rel.contains("icon") && !link_href.is_empty() {
            let mut href = link_href.to_string();
            if !link_href.starts_with("http") {
                if link_href.contains("base64,") {
                    log::debug!("base64 icons are not supported");
                    continue;
                } else {
                    let mut url_parsed = reqwest::Url::parse(url).unwrap();
                    url_parsed.set_path(link_href);
                    href = url_parsed.to_string();
                }
            }

            icons.push((href, link_rel, link_type, link_sizes))
        }
    }

    icons.sort_by(|a, b| {
        let (a_link_href, _, _, _) = a;
        let (b_link_href, _, _, _) = b;
        if a_link_href.contains(".ico") && !b_link_href.contains(".ico") {
            return Ordering::Greater;
        }

        if !a_link_href.contains(".ico") && b_link_href.contains(".ico") {
            return Ordering::Less;
        }

        if a_link_href.contains(".png") && !b_link_href.contains(".png") {
            return Ordering::Less;
        }

        if !a_link_href.contains(".png") && b_link_href.contains(".png") {
            return Ordering::Greater;
        }

        Ordering::Equal
    });

    icon_url = icons.first().map(|icon| icon.0.clone());

    // try to get favicon from duckduckgo
    if icon_url.is_none() {
        let url_parsed = reqwest::Url::parse(url).unwrap();
        let host = url_parsed.host_str();

        if let Some(host) = host {
            icon_url = Some(format!(
                "https://external-content.duckduckgo.com/ip3/{host}.ico"
            ));
        }
    }

    for element in head.select(&title_selector) {
        let title_text = element.text().next().unwrap_or_default();
        title = Some(title_text.to_string());
    }

    if let Some(ref img) = icon_url {
        if !img.starts_with("http") {
            let mut url_parsed = reqwest::Url::parse(url).unwrap();
            url_parsed.set_path(&img);
            icon_url = Some(url_parsed.to_string());
        }
    }

    if let Some(ref img) = image_url {
        if !img.starts_with("http") {
            let mut url_parsed = reqwest::Url::parse(url).unwrap();
            url_parsed.set_path(&img);
            image_url = Some(url_parsed.to_string());
        }
    }

    Metadata {
        title,
        description,
        keywords,
        canonical_url,
        image_url,
        icon_url,
        image: None,
        icon: None,
        dump: None,
    }
}
