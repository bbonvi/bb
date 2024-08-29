use crate::{eid::Eid, storage::StorageManager};
use core::panic;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{read_to_string, rename, write},
    hash::Hash,
    io::ErrorKind,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone, Eq, Default, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: u64,

    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub url: String,

    pub image_id: Option<String>,
    pub icon_id: Option<String>,
}

impl Hash for Bookmark {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl PartialEq for Bookmark {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BookmarkCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    pub url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BookmarkUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchQuery {
    pub id: Option<u64>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,

    #[serde(default)]
    pub exact: bool,

    #[serde(default)]
    pub limit: Option<usize>,
}

pub trait BookmarkManager: Send + Sync {
    fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<Bookmark>>;
    fn total(&self) -> anyhow::Result<usize>;
    fn create(&self, bmark_create: BookmarkCreate) -> anyhow::Result<Bookmark>;
    fn delete(&self, id: u64) -> anyhow::Result<Option<bool>>;
    fn update(&self, id: u64, bmark_update: BookmarkUpdate) -> anyhow::Result<Option<Bookmark>>;

    fn search_delete(&self, query: SearchQuery) -> anyhow::Result<usize>;
    fn search_update(
        &self,
        query: SearchQuery,
        bmark_update: BookmarkUpdate,
    ) -> anyhow::Result<usize>;

    fn refresh(&self) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct BackendJson {
    list: Arc<RwLock<Vec<Bookmark>>>,
    path: String,
}

impl BackendJson {
    pub fn load(path: &str) -> Self {
        let bookmarks_plain = match read_to_string(path) {
            Ok(b) => b,
            Err(err) => match err.kind() {
                ErrorKind::NotFound => "[]".to_string(),
                _ => panic!("{err}"),
            },
        };
        let bookmarks: Vec<Bookmark> = serde_json::from_str(&bookmarks_plain).unwrap();
        // let mut seen = HashSet::new();
        // bookmarks.retain(|item| seen.insert(item.clone()));
        let mgr = BackendJson {
            list: Arc::new(RwLock::new(bookmarks)),
            path: path.to_string(),
        };

        // mgr.save();

        mgr
    }

    pub fn save(&self) {
        let store = crate::storage::BackendLocal::new("./");

        let bmarks = self.list.write().unwrap();

        store.write(
            &self.path,
            &serde_json::to_string(&*bmarks).unwrap().as_bytes(),
        );
    }

    #[cfg(test)]
    pub fn wipe_database(self) -> Self {
        let _ = std::fs::remove_file(&self.path);
        *self.list.write().unwrap() = vec![];
        self
    }
}

impl BookmarkManager for BackendJson {
    fn create(&self, bmark_create: BookmarkCreate) -> anyhow::Result<Bookmark> {
        let id = if let Some(last_bookmark) = self.list.write().unwrap().last() {
            last_bookmark.id + 1
        } else {
            0
        };

        let mut bmark_create = bmark_create;
        if let Some(ref mut tags) = bmark_create.tags {
            let mut seen = HashSet::new();
            tags.retain(|item| seen.insert(item.clone()));
        };

        let bmark = Bookmark {
            id,
            title: bmark_create.title.unwrap_or_default(),
            description: bmark_create.description.unwrap_or_default(),
            tags: bmark_create.tags.unwrap_or_default(),
            url: bmark_create.url,
            image_id: bmark_create.image_id,
            icon_id: bmark_create.icon_id,
        };

        self.list.write().unwrap().push(bmark.clone());

        self.save();

        Ok(bmark)
    }

    fn search_delete(&self, query: SearchQuery) -> anyhow::Result<usize> {
        let results = self.search(query)?;
        let mut delete_ids = results;
        let count = delete_ids.len();

        let mut bmarks = self.list.write().unwrap();
        *bmarks = bmarks
            .iter()
            .filter(|b| delete_ids.iter().find(|bb| b.id == bb.id).is_none())
            .cloned()
            .collect::<Vec<Bookmark>>();

        drop(bmarks);

        self.save();

        Ok(count)
    }

    fn search_update(
        &self,
        query: SearchQuery,
        bmark_update: BookmarkUpdate,
    ) -> anyhow::Result<usize> {
        let results = self.search(query)?;
        let count = results.len();
        let mut bmarks = self.list.write().unwrap();
        for bmark in bmarks.iter_mut() {
            if results.iter().find(|b| b.id == bmark.id).is_none() {
                continue;
            }

            if let Some(ref title) = bmark_update.title {
                bmark.title = title.to_string();
            }
            if let Some(ref descr) = bmark_update.description {
                bmark.description = descr.to_string();
            }
            if let Some(ref tags) = bmark_update.tags {
                bmark.tags = tags.to_vec();
                let mut seen = HashSet::new();
                bmark.tags.retain(|item| seen.insert(item.clone()));
            }
            if let Some(ref url) = bmark_update.url {
                bmark.url = url.to_string();
            }

            if let Some(ref image_id) = bmark_update.image_id {
                bmark.image_id = Some(image_id.to_string());
            }
            if let Some(ref icon_id) = bmark_update.icon_id {
                bmark.icon_id = Some(icon_id.to_string());
            }
        }

        drop(bmarks);

        self.save();

        Ok(count)
    }

    fn delete(&self, id: u64) -> anyhow::Result<Option<bool>> {
        let mut bmarks = self.list.write().unwrap();
        let result = bmarks.iter().position(|b| b.id == id).map(|idx| {
            bmarks.remove(idx);
            true
        });

        drop(bmarks);

        if result.is_some() {
            self.save();
        }

        Ok(result)
    }

    fn update(&self, id: u64, bmark_update: BookmarkUpdate) -> anyhow::Result<Option<Bookmark>> {
        let mut bmarks = self.list.write().unwrap();

        let bmark = if let Some(bmark) = bmarks.iter_mut().find(|b| b.id == id) {
            if let Some(title) = bmark_update.title {
                bmark.title = title;
            }
            if let Some(descr) = bmark_update.description {
                bmark.description = descr;
            }
            if let Some(tags) = bmark_update.tags {
                bmark.tags = tags;
                let mut seen = HashSet::new();
                bmark.tags.retain(|item| seen.insert(item.clone()));
            }
            if let Some(url) = bmark_update.url {
                bmark.url = url;
            }

            if let Some(image_id) = bmark_update.image_id {
                bmark.image_id = Some(image_id);
            }
            if let Some(icon_id) = bmark_update.icon_id {
                bmark.icon_id = Some(icon_id);
            }
            Some(bmark.clone())
        } else {
            None
        };

        drop(bmarks);

        self.save();

        Ok(bmark)
    }

    fn total(&self) -> anyhow::Result<usize> {
        let bmarks = self.list.read().unwrap();
        Ok(bmarks.len())
    }

    fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<Bookmark>> {
        let bmarks = self.list.read().unwrap();

        let mut query = query;
        query.lowercase();

        let mut output = vec![];

        // return all
        if query.description.is_none()
            && query.url.is_none()
            && query.title.is_none()
            && (query.tags.is_none() || query.tags.clone().unwrap_or_default().is_empty())
            && query.id.is_none()
        {
            return Ok(bmarks.clone());
        }

        let query_tags = query.tags.map(|tags| {
            let tags = tags.clone();
            tags.iter()
                .cloned()
                .map(|tag| {
                    let unprefixed = tag.strip_prefix("-").map(String::from);
                    (
                        tag.clone(),
                        format!("{tag}/"),
                        unprefixed.clone(),
                        format!("{}/", unprefixed.unwrap_or_default()),
                    )
                })
                .collect::<Vec<_>>()
        });

        for bookmark in bmarks.iter() {
            let mut has_match = false;

            if let Some(id) = &query.id {
                if bookmark.id == *id {
                    has_match = true;
                } else {
                    continue;
                }
            };

            if let Some(url) = &query.url {
                if query.exact && bookmark.url.eq_ignore_ascii_case(url)
                    || !query.exact && bookmark.url.to_lowercase().contains(url)
                {
                    has_match = true;
                } else {
                    continue;
                }
            };

            if let Some(description) = &query.description {
                if query.exact && bookmark.description.eq_ignore_ascii_case(description)
                    || !query.exact && bookmark.description.to_lowercase().contains(description)
                {
                    has_match = true;
                } else {
                    continue;
                }
            };

            if let Some(title) = &query.title {
                if query.exact && bookmark.title.eq_ignore_ascii_case(title)
                    || !query.exact && bookmark.title.to_lowercase().contains(title)
                {
                    has_match = true;
                } else {
                    continue;
                }
            };

            let bmark_tags = bookmark
                .tags
                .iter()
                .map(|t| t.to_lowercase())
                .collect::<Vec<_>>();

            if let Some(tags) = &query_tags {
                for (tag, teg_delim, neg_tag, neg_tag_delim) in tags {
                    let mut bmark_tags = bmark_tags.iter();
                    if let Some(neg_tag) = neg_tag {
                        if bmark_tags.any(|tag_b| neg_tag == tag_b || tag_b.contains(neg_tag_delim))
                        {
                            has_match = false;
                            break;
                        } else {
                            has_match = true;
                        }
                    } else {
                        if !bmark_tags.any(|tag_b| tag == tag_b || tag_b.contains(teg_delim)) {
                            has_match = false;
                            break;
                        } else {
                            has_match = true;
                        }
                    }
                }
            };

            if has_match {
                output.push(bookmark.clone());
            }

            // early return because we know there will be no matches after that point
            // if has_match && (query.id.is_some() || (query.url.is_some() && query.exact))
            if has_match && query.id.is_some()
                || query.limit.is_some() && query.limit.unwrap_or_default() >= output.len()
            {
                break;
            }
        }

        Ok(output)
    }

    fn refresh(&self) -> anyhow::Result<()> {
        let mut list = self.list.write().unwrap();
        let backend = Self::load(&self.path);
        *list = backend.list.write().unwrap().clone();
        Ok(())
    }
}

impl SearchQuery {
    pub fn lowercase(&mut self) {
        self.title = self.title.as_ref().map(|title| title.to_lowercase());
        self.description = self
            .description
            .as_ref()
            .map(|description| description.to_lowercase());
        self.url = self.url.as_ref().map(|url| url.to_lowercase());
        self.tags = self
            .tags
            .as_ref()
            .map(|tags| tags.iter().map(|t| t.to_lowercase()).collect::<Vec<_>>());
    }
}

#[cfg(test)]
impl BackendJson {
    #[cfg(test)]
    pub fn list(&self) -> Arc<RwLock<Vec<Bookmark>>> {
        self.list.clone()
    }
}
