use headless_chrome::{
    protocol::cdp::{Page, Target::CreateTarget},
    LaunchOptionsBuilder, Tab,
};
use std::{path::PathBuf, str::FromStr, sync::Arc, thread::sleep, time::Duration};

pub struct ChromeResult {
    pub html: String,
    pub screenshot: Vec<u8>,
}

fn looks_like_challenge(tab: Arc<Tab>) -> bool {
    // Cheap signals, no bypassing: detect “Just a moment…”/challenge iframes.
    let title_ok = tab.get_title().unwrap_or_default();
    if title_ok.to_lowercase().contains("just a moment") {
        return true;
    }

    let res = tab.evaluate(
        r#"
        !!(document.querySelector('iframe[src*="challenge"]')
           || document.querySelector('div[id*="cf-"], div[class*="cf-"]')
           || document.querySelector('iframe[src*="hcaptcha"], iframe[src*="turnstile"]'))
    "#,
        false,
    );
    res.ok()
        .and_then(|v| v.value.and_then(|x| x.as_bool()))
        .unwrap_or(false)
}

pub fn stealth_tab(tab: Arc<Tab>) {
    tab.call_method(Page::AddScriptToEvaluateOnNewDocument {
        run_immediately: Some(true),
        source: "Object.defineProperty(navigator, 'webdriver', {get: () => undefined});"
            .to_string(),
        world_name: None,
        include_command_line_api: None,
    })
    .unwrap();
    tab.set_user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36", Some("en-US,en"), Some("Mac OS X")).unwrap();

    tab.call_method(Page::AddScriptToEvaluateOnNewDocument {
        run_immediately: Some(true),
        source: include_str!("./stealth_scripts.js").to_string(),
        world_name: None,
        include_command_line_api: None,
    })
    .unwrap();
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

pub fn test_launch() {
    headless_chrome::Browser::new(
        LaunchOptionsBuilder::default()
            .sandbox(false)
            .path(
                std::env::var("CHROME_PATH")
                    .ok()
                    .map(|p| PathBuf::from_str(&p).unwrap()),
            )
            .build()
            .unwrap(),
    )
    .expect("could not launch chromium");
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
                .sandbox(false)
                .proxy_server(proxy.as_deref())
                .path(
                    std::env::var("CHROME_PATH")
                        .ok()
                        .map(|p| PathBuf::from_str(&p).unwrap()),
                )
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
                for_tab: None,
                url: url.to_string(),
                width: Some(1280),
                height: Some(720),
                browser_context_id: None,
                enable_begin_frame_control: None,
                new_window: Some(true),
                background: None,
            })
            .unwrap();

        tab.enable_stealth_mode().unwrap();
        stealth_tab(tab.clone());

        tab.set_default_timeout(Duration::from_secs(15));

        if let Err(err) = tab.navigate_to(url) {
            log::error!("{host}: {err}");
            force_proxy = true;
            continue;
        }

        if let Err(err) = tab.wait_until_navigated() {
            log::error!("{host}: {}", err);
            force_proxy = true;
            continue;
        }

        log::debug!("{host}: sleeping for 2 seconds...");
        sleep(Duration::from_secs(5));

        // in case the page hasn't been fully loaded yet;
        log::debug!("{host}: sleeping some more...");
        let _ = tab.wait_for_element_with_custom_timeout("body * *", Duration::from_secs(10));

        if looks_like_challenge(tab.clone()) {
            log::info!("challenge detected");
            return None;
        }

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
