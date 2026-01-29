use crate::config::ScrapeConfig;
use crate::metadata::fetchers::MetadataFetcher;
use crate::metadata::types::Metadata;

pub struct DdgFetcher;

impl DdgFetcher {
    pub fn new() -> Self {
        Self
    }
}

impl MetadataFetcher for DdgFetcher {
    fn fetch(
        &self,
        url: &str,
        scrape_config: Option<&ScrapeConfig>,
    ) -> anyhow::Result<Option<Metadata>> {
        Ok(crate::scrape::get_data_from_ddg(url, scrape_config))
    }

    fn name(&self) -> &'static str {
        "DDG"
    }
}
