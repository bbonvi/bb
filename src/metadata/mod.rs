pub mod types;
pub mod fetchers;
pub mod normalize;
pub mod image_validation;

pub use types::{Metadata, MetaOptions, MetadataReport, FieldDecision};
pub use fetchers::FetcherRegistry;

use anyhow::Result;

/// Main entry point for fetching metadata from a URL
pub fn fetch_meta(url: &str, opts: MetaOptions) -> Result<(Metadata, MetadataReport)> {
    let url = &normalize::normalize_url(url);
    let scrape_config = opts.scrape_config.as_ref();
    let registry = FetcherRegistry::new(&opts);

    let (meta_opt, mut report) = registry.fetch_metadata(url, &opts)?;
    let mut metadata = meta_opt.unwrap_or_default();

    if metadata.image.is_none() {
        metadata.try_fetch_image(scrape_config);
    }
    if metadata.icon.is_none() {
        metadata.try_fetch_icon(scrape_config);
    }
    if metadata.icon.is_none() {
        let had_icon = metadata.icon.is_some();
        try_fetch_ddg_favicon(&mut metadata, url, scrape_config);
        if !had_icon && metadata.icon.is_some() {
            report.field_decisions.push(FieldDecision {
                field: "icon".into(),
                winner: "DDG favicon".into(),
                reason: "fallback favicon from DuckDuckGo".into(),
                value_preview: metadata.icon_url.clone(),
            });
        }
    }

    Ok((metadata, report))
}

/// Try to fetch favicon from DuckDuckGo if icon is missing
fn try_fetch_ddg_favicon(meta: &mut Metadata, url: &str, scrape_config: Option<&crate::config::ScrapeConfig>) {
    if meta.icon.is_none() {
        log::debug!("attempting DuckDuckGo favicon");
        if let Ok(parsed) = reqwest::Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                let icon_url = format!("https://external-content.duckduckgo.com/ip3/{host}.ico");
                if let Some(bytes) = fetchers::fetch_bytes(&icon_url, scrape_config) {
                    log::debug!("favicon fetched from DuckDuckGo");
                    meta.icon = Some(bytes);
                    meta.icon_url = Some(icon_url);
                }
            }
        }
    }
}
