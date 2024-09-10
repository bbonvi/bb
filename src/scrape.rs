use crate::metadata::Metadata;
use headless_chrome::{
    protocol::cdp::{Page, Target::CreateTarget},
    LaunchOptionsBuilder, Tab,
};
use reqwest::StatusCode;
use std::{
    cmp::Ordering, error::Error, path::PathBuf, str::FromStr, sync::Arc, thread::sleep,
    time::Duration,
};

const _USER_AGENT_GOOGLE: &'static str = "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/W.X.Y.Z Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
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

pub fn apply_css_rules(body: &headless_chrome::browser::tab::element::Element) {
    let styles = include_str!("./styles.css");
    let _ = body.call_js_fn(
        &format!(
            r#"
            function apply_css() {{
                function injectCSS(cssString) {{
                    const style = document.createElement('style');
                    style.type = 'text/css';
                    if (style.styleSheet) {{
                        style.styleSheet.cssText = cssString;
                    }} else {{
                        style.appendChild(document.createTextNode(cssString));
                    }}
                    document.head.appendChild(style);
                }}
                injectCSS(`{styles}`);
            }}
        "#
        ),
        vec![],
        false,
    );
}
pub fn stealth_tab(tab: Arc<Tab>) {
    tab.call_method(Page::AddScriptToEvaluateOnNewDocument {
        source: "Object.defineProperty(navigator, 'webdriver', {get: () => undefined});"
            .to_string(),
        world_name: None,
        include_command_line_api: None,
    })
    .unwrap();
    tab.set_user_agent("'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36'", Some("en-US,en"), Some("Mac OS X")).unwrap();

    tab.call_method(Page::AddScriptToEvaluateOnNewDocument {
        source: include_str!("./stealth_scripts.js").to_string(),
        world_name: None,
        include_command_line_api: None,
    })
    .unwrap();
}

pub struct ChromeResult {
    pub html: String,
    pub screenshot: Vec<u8>,
}

pub fn fetch_page_with_chrome(url: &str) -> Option<ChromeResult> {
    let opt_proxy = std::env::var("OPT_PROXY").unwrap_or_default();

    let url_parsed = reqwest::Url::parse(url).unwrap();
    let host = url_parsed.host_str().unwrap_or_default();

    let mut r = 0;
    let mut force_proxy = false;
    loop {
        r += 1;

        if r >= 5 {
            return None;
        }

        if r > 0 {
            log::debug!("{host}: retrying");
        }

        r += 1;

        let proxy = if force_proxy {
            Some(opt_proxy.clone())
        } else {
            None
        };

        let browser = match headless_chrome::Browser::new(
            LaunchOptionsBuilder::default()
                .proxy_server(proxy.as_deref())
                .path(Some(
                    PathBuf::from_str(
                        &std::env::var("CHROME_PATH").unwrap_or("chromium".to_string()),
                    )
                    .unwrap(),
                ))
                .build()
                .unwrap(),
        ) {
            Ok(b) => b,
            Err(err) => {
                log::error!("failed to start chrome: {}", err);
                return None;
            }
        };

        let tab = browser
            .new_tab_with_options(CreateTarget {
                url: url.to_string(),
                width: Some(1366),
                height: Some(768),
                browser_context_id: None,
                enable_begin_frame_control: None,
                new_window: None,
                background: None,
            })
            .unwrap();

        stealth_tab(tab.clone());

        tab.set_default_timeout(Duration::from_secs(10));

        if let Err(err) = tab.navigate_to(url) {
            log::error!("{host}: {}", err);
            force_proxy = true;
            continue;
        }

        if let Err(err) = tab.wait_until_navigated() {
            log::error!("{host}: {}", err);
            force_proxy = true;
            continue;
        }

        sleep(Duration::from_secs(2));

        if let Ok(body) = tab.wait_for_element("body") {
            apply_css_rules(&body);

            // remove every element on page that has position: fixed property;
            // this is to combat any potential popups and alerts
            let _ = body.call_js_fn(
                r#"
                    function remove_fixed() {
                        function clear() {
                            // document.querySelectorAll("*").forEach(el =>  {
                            //     if (el.computedStyleMap().get("position")?.value === "fixed") { 
                            //         el.outerHTML = "";
                            //     }
                            // })
                        }

                        clear();

                        setInterval(() => {
                            clear()
                        }, 5);
                    }
                "#,
                vec![],
                false,
            );
        }

        let html = tab.get_content().unwrap();

        let png_data = tab
            .capture_screenshot(Page::CaptureScreenshotFormatOption::Png, None, None, true)
            .unwrap();

        let _ = tab.close(true);

        return Some(ChromeResult {
            screenshot: png_data,
            html,
        });
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

pub fn guess_filetype(url: &str) -> Option<String> {
    if url.contains(".jpg") || url.contains(".jpeg") {
        return Some(String::from("jpg"));
    }

    if url.contains(".webp") {
        return Some(String::from("webp"));
    }

    if url.contains(".png") {
        return Some(String::from("png"));
    }

    if url.contains(".svg") {
        return Some(String::from("svg"));
    }

    if url.contains(".bmp") {
        return Some(String::from("bmp"));
    }

    if url.contains(".gif") {
        return Some(String::from("gif"));
    }

    None
}
