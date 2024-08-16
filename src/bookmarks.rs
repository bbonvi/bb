use crate::eid::Eid;
use core::panic;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{read_to_string, rename, write},
    io::ErrorKind,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: u64,

    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub url: String,

    pub image_id: Option<String>,
    pub icon_id: Option<String>,
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
    pub no_exact_url: bool,
}

pub trait BookmarkManager: Send + Sync {
    fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<Bookmark>>;
    fn create(&self, bmark_create: BookmarkCreate) -> anyhow::Result<Bookmark>;
    fn delete(&self, id: u64) -> anyhow::Result<Option<bool>>;
    fn update(&self, id: u64, bmark_update: BookmarkUpdate) -> anyhow::Result<Option<Bookmark>>;
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
        let bookmarks = serde_json::from_str(&bookmarks_plain).unwrap();
        let mgr = Self {
            list: Arc::new(RwLock::new(bookmarks)),
            path: path.to_string(),
        };

        mgr.save();

        mgr
    }

    pub fn save(&self) {
        let bmarks = self.list.read().unwrap();

        let temp_path = format!("{}-{}", &self.path, Eid::new());

        write(&temp_path, serde_json::to_string(&*bmarks).unwrap()).unwrap();
        rename(&temp_path, &self.path).unwrap();
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

        let query = SearchQuery {
            url: Some(bmark_create.url.clone()),
            ..Default::default()
        };

        if let Some(b) = self.search(query)?.first() {
            // bail!("bookmark with this url already exists at index {0}", b.id);
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
        let result = self
            .list
            .write()
            .unwrap()
            .iter_mut()
            .find(|b| b.id == id)
            .map(|b| {
                if let Some(title) = bmark_update.title {
                    b.title = title;
                }
                if let Some(descr) = bmark_update.description {
                    b.description = descr;
                }
                if let Some(tags) = bmark_update.tags {
                    b.tags = tags;
                    let mut seen = HashSet::new();
                    b.tags.retain(|item| seen.insert(item.clone()));
                }
                if let Some(url) = bmark_update.url {
                    b.url = url;
                }

                if let Some(image_id) = bmark_update.image_id {
                    b.image_id = Some(image_id);
                }
                if let Some(icon_id) = bmark_update.icon_id {
                    b.icon_id = Some(icon_id);
                }

                b.clone()
            });

        if result.is_some() {
            self.save();
        }

        Ok(result)
    }

    fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<Bookmark>> {
        let bmarks = self.list.read().unwrap();

        let mut query = query;
        query.lowercase();

        Ok(bmarks
            .iter()
            .filter(|bookmark| {
                if query.description.is_none()
                    && query.url.is_none()
                    && query.title.is_none()
                    && (query.tags.is_none() || query.tags.clone().unwrap_or_default().is_empty())
                    && query.id.is_none()
                {
                    return true;
                }

                let mut has_match = false;
                if let Some(id) = &query.id {
                    if bookmark.id == *id {
                        has_match = true;
                    } else {
                        return false;
                    }
                };

                if let Some(url) = &query.url {
                    let bmark_url = bookmark.url.to_lowercase();
                    if !query.no_exact_url && bmark_url == *url
                        || query.no_exact_url && bmark_url.contains(url)
                    {
                        has_match = true;
                    } else {
                        return false;
                    }
                };

                if let Some(description) = &query.description {
                    let bmark_description = bookmark.description.to_lowercase();
                    if query.exact && bmark_description == *description
                        || !query.exact && bmark_description.contains(description)
                    {
                        has_match = true;
                    } else {
                        return false;
                    }
                };

                if let Some(title) = &query.title {
                    let bmark_title = bookmark.title.to_lowercase();
                    if query.exact && bmark_title == *title
                        || !query.exact && bmark_title.contains(title)
                    {
                        has_match = true;
                    } else {
                        return false;
                    }
                };

                if let Some(tags) = &query.tags {
                    let mut bmark_tags = bookmark.tags.iter().map(|t| t.to_lowercase());
                    for tag in tags {
                        match bmark_tags.find(|tag_b| tag == tag_b).is_none() {
                            true => {
                                has_match = false;
                                break;
                            }
                            false => {
                                has_match = true;
                            }
                        }
                    }
                };

                return has_match;
            })
            .map(|b| b.clone())
            .collect::<Vec<_>>())
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
