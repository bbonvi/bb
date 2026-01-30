use std::sync::RwLock;

use anyhow::bail;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::{bookmarks, config::{Config, RulesConfig}, rules::Rule, web::TotalResponse};

use super::{backend::*, errors::AppError};

pub struct AppRemote {
    remote_addr: String,
    basic_auth: Option<(String, Option<String>)>,
    bearer_token: Option<String>,
    client: reqwest::blocking::Client,
}

impl AppRemote {
    pub fn new(
        addr: &str,
        basic_auth: Option<(String, Option<String>)>,
        bearer_token: Option<String>,
    ) -> AppRemote {
        let remote_addr = addr.strip_suffix("/").unwrap_or(addr).to_string();
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        AppRemote {
            remote_addr,
            basic_auth,
            bearer_token,
            client,
        }
    }

    fn get(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        log::info!("{}{}", self.remote_addr, url);
        let url = format!("{}{}", self.remote_addr, url);
        let request = self.client.get(&url);

        self.attach_auth(request)
    }

    fn post(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        log::info!("{}{}", self.remote_addr, url);
        let url = format!("{}{}", self.remote_addr, url);
        let request = self.client.post(&url);

        self.attach_auth(request)
    }

    fn attach_auth(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        // Bearer token takes precedence over basic auth
        if let Some(ref token) = self.bearer_token {
            request.bearer_auth(token)
        } else if let Some((ref username, ref password)) = self.basic_auth {
            request.basic_auth(username, password.clone())
        } else {
            request
        }
    }

    // pub fn fetch_metadata(&self, url: &str, opts: FetchMetadataOpts) -> anyhow::Result<Metadata> {
    //     let metadata: Metadata = self
    //         .post("/api/bookmarks/fetch_metadata")
    //         .json(&json!({
    //             "url": url,
    //             "opts": opts,
    //         }))
    //         .send()?
    //         .json()?;
    //
    //     Ok(metadata)
    // }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum WebResponse<T> {
    Error { error: String },
    Data(T),
}

fn handle_response<T>(response: reqwest::blocking::Response) -> anyhow::Result<T>
where
    T: DeserializeOwned + Clone,
{
    let text = response.text()?;

    let web_response = serde_json::from_str::<WebResponse<T>>(&text).map_err(|err| {
        log::error!("{err}. tried to parse: {text:?}");
        err
    })?;

    match web_response {
        WebResponse::Data(data) => Ok(data),
        WebResponse::Error { error } => {
            bail!(error)
        }
    }
}

impl AppBackend for AppRemote {
    fn config(&self) -> anyhow::Result<Arc<RwLock<Config>>, AppError> {
        let resp = self.get("/api/config").send()?;
        Ok(handle_response::<Config>(resp).map(|c| Arc::new(RwLock::new(c)))?)
    }
    fn update_config(&self, config: Config) -> anyhow::Result<(), AppError> {
        let resp = self.post("/api/config").json(&config).send()?;

        Ok(handle_response(resp)?)
    }
    fn rules(&self) -> anyhow::Result<Arc<RwLock<RulesConfig>>, AppError> {
        let resp = self.get("/api/rules").send()?;
        let rules: Vec<Rule> = handle_response(resp)?;
        Ok(Arc::new(RwLock::new(RulesConfig::from_rules(rules))))
    }
    fn refresh_metadata(&self, id: u64, opts: RefreshMetadataOpts) -> anyhow::Result<Option<crate::metadata::MetadataReport>, AppError> {
        let resp = self
            .post("/api/bookmarks/refresh_metadata")
            .json(&json!({
                "id": id,
                "async_meta": opts.async_meta,
                "no_headless": opts.meta_opts.no_headless,
            }))
            .send()?;

        #[derive(Deserialize, Clone)]
        struct RefreshResponse {
            report: Option<crate::metadata::MetadataReport>,
        }

        let parsed: RefreshResponse = handle_response(resp)?;
        Ok(parsed.report)
    }

    fn create(
        &self,
        bmark_create: bookmarks::BookmarkCreate,
        opts: AddOpts,
    ) -> anyhow::Result<(bookmarks::Bookmark, Option<crate::metadata::MetadataReport>), AppError> {
        #[derive(Deserialize, Clone)]
        struct CreateResponse {
            #[serde(flatten)]
            bookmark: bookmarks::Bookmark,
            report: Option<crate::metadata::MetadataReport>,
        }

        let resp = self
            .post("/api/bookmarks/create")
            .json(&json!({
                "title": bmark_create.title,
                "description": bmark_create.description,
                "tags": bmark_create.tags.map(|t| t.join(",")),
                "url": bmark_create.url,
                "async_meta": opts.async_meta,
                "no_meta": opts.meta_opts.is_none(),
                "no_headless": opts.meta_opts.unwrap_or_default().no_headless,
            }))
            .send()?;

        let parsed: CreateResponse = handle_response(resp)?;
        Ok((parsed.bookmark, parsed.report))
    }

    fn update(
        &self,
        id: u64,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<bookmarks::Bookmark, AppError> {
        let resp = self
            .post("/api/bookmarks/update")
            .json(&json!({
                "id": id,
                "title": bmark_update.title,
                "description": bmark_update.description,
                "tags": bmark_update.tags.map(|t| t.join(",")),
                "append_tags": bmark_update.append_tags.map(|t| t.join(",")),
                "remove_tags": bmark_update.remove_tags.map(|t| t.join(",")),
                "url": bmark_update.url,
                "image_b64": bmark_update.image_id,
                "icon_b64": bmark_update.icon_id,
            }))
            .send()?;

        Ok(handle_response(resp)?)
    }

    fn delete(&self, id: u64) -> anyhow::Result<(), AppError> {
        let resp = self
            .post("/api/bookmarks/delete")
            .json(&json!({
                "id": id,
            }))
            .send()?;

        Ok(handle_response(resp)?)
    }

    fn search_delete(&self, query: bookmarks::SearchQuery) -> anyhow::Result<usize, AppError> {
        let resp = self
            .post("/api/bookmarks/search_delete")
            .json(&query)
            .send()?;

        Ok(handle_response(resp)?)
    }

    fn search_update(
        &self,
        query: bookmarks::SearchQuery,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<usize, AppError> {
        let resp = self
            .post("/api/bookmarks/search_update")
            .json(&json!({
                "query": query,
                "update": bmark_update,
            }))
            .send()?;

        Ok(handle_response(resp)?)
    }

    fn total(&self) -> anyhow::Result<usize, AppError> {
        let resp = self.post("/api/bookmarks/total").send()?;
        let resp = handle_response::<TotalResponse>(resp)?;

        Ok(resp.total)
    }

    fn search(
        &self,
        query: bookmarks::SearchQuery,
    ) -> anyhow::Result<Vec<bookmarks::Bookmark>, AppError> {
        log::debug!("search: {query:?}");
        let resp = self
            .post("/api/bookmarks/search")
            .json(&json!({
                "id": query.id,
                "title": query.title,
                "url": query.url,
                "description": query.description,
                "tags": query.tags.map(|tags| tags.join(",")),
                "query": query.query,
                "semantic": query.semantic,
                "threshold": query.threshold,
                "exact": query.exact,
                "limit": query.limit
            }))
            .send()?;

        Ok(handle_response(resp)?)
    }

    fn bookmark_version(&self) -> u64 {
        0
    }

    fn tags(&self) -> anyhow::Result<Vec<String>, AppError> {
        let resp = self.post("/api/bookmarks/tags").send()?;

        Ok(handle_response(resp)?)
    }
}
