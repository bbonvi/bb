use std::sync::RwLock;

use anyhow::bail;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::{bookmarks, config::Config, web::TotalResponse};

use super::{backend::*, errors::AppError};

pub struct AppRemote {
    remote_addr: String,
    basic_auth: Option<(String, Option<String>)>,
}

impl AppRemote {
    pub fn new(addr: &str, basic_auth: Option<(String, Option<String>)>) -> AppRemote {
        let remote_addr = addr.strip_suffix("/").unwrap_or(addr).to_string();

        AppRemote {
            remote_addr,
            basic_auth,
        }
    }

    fn get(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        log::info!("{}{}", self.remote_addr, url);
        let url = format!("{}{}", self.remote_addr, url);

        match self.basic_auth.clone() {
            Some((username, password)) => reqwest::blocking::Client::new()
                .get(&url)
                .basic_auth(username, password),
            None => reqwest::blocking::Client::new().get(&url),
        }
    }

    fn post(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        log::info!("{}{}", self.remote_addr, url);
        let url = format!("{}{}", self.remote_addr, url);

        match self.basic_auth.clone() {
            Some((username, password)) => reqwest::blocking::Client::new()
                .post(&url)
                .basic_auth(username, password),
            None => reqwest::blocking::Client::new().post(&url),
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
    fn refresh_metadata(&self, id: u64, opts: RefreshMetadataOpts) -> anyhow::Result<(), AppError> {
        let resp = self
            .post("/api/bookmarks/refresh_metadata")
            .json(&json!({
                "id": id,
                "async_meta": opts.async_meta,
                "no_headless": opts.meta_opts.no_headless,
            }))
            .send()?;

        Ok(handle_response(resp)?)
    }

    fn create(
        &self,
        bmark_create: bookmarks::BookmarkCreate,
        opts: AddOpts,
    ) -> anyhow::Result<bookmarks::Bookmark, AppError> {
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

        Ok(handle_response(resp)?)
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
                "url": bmark_update.url,
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
                "exact": query.exact
            }))
            .send()?;

        Ok(handle_response(resp)?)
    }

    fn tags(&self) -> anyhow::Result<Vec<String>, AppError> {
        let resp = self.post("/api/bookmarks/tags").send()?;

        Ok(handle_response(resp)?)
    }
}
