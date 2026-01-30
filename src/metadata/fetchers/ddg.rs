use crate::config::ScrapeConfig;
use crate::metadata::fetchers::MetadataFetcher;
use crate::metadata::types::FetchOutcome;

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
    ) -> anyhow::Result<FetchOutcome> {
        match crate::scrape::get_data_from_ddg(url, scrape_config) {
            Some(m) => Ok(FetchOutcome::Data(m)),
            None => Ok(FetchOutcome::Skip("DDG returned no data".into())),
        }
    }

    fn name(&self) -> &'static str {
        "DDG"
    }
}
