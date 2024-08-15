use anyhow::bail;
use core::panic;
use serde::{Deserialize, Serialize};
use std::{
    borrow::BorrowMut,
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

pub trait BookmarkMgrBackend: Send + Sync {
    fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<Bookmark>>;
    fn add(&self, bookmark: BookmarkCreate) -> anyhow::Result<Bookmark>;
    fn delete(&self, id: u64) -> anyhow::Result<Option<bool>>;
    fn update(&self, id: u64, bmark_update: BookmarkUpdate) -> anyhow::Result<Option<Bookmark>>;
}

#[derive(Debug, Clone, Default)]
pub struct BookmarkMgrJson {
    pub bookmarks: Arc<RwLock<Vec<Bookmark>>>,
}

impl BookmarkMgrJson {
    pub fn load() -> Self {
        let bookmarks_plain = match read_to_string("bookmarks.json") {
            Ok(b) => b,
            Err(err) => match err.kind() {
                ErrorKind::NotFound => "[]".to_string(),
                _ => {
                    panic!("{err}");
                }
            },
        };
        let bookmarks = serde_json::from_str(&bookmarks_plain).unwrap();
        Self {
            bookmarks: Arc::new(RwLock::new(bookmarks)),
        }
    }
    pub fn save(&self) {
        let bmarks = self.bookmarks.read().unwrap();
        write("bookmarks-.json", serde_json::to_string(&*bmarks).unwrap()).unwrap();
        rename("bookmarks-.json", "bookmarks.json").unwrap();
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

impl BookmarkMgrBackend for BookmarkMgrJson {
    fn add(&self, bmark_create: BookmarkCreate) -> anyhow::Result<Bookmark> {
        let id = if let Some(last_bookmark) = self.bookmarks.write().unwrap().last() {
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

        let bookmark = Bookmark {
            id,
            title: bmark_create.title.unwrap_or_default(),
            description: bmark_create.description.unwrap_or_default(),
            tags: bmark_create.tags.unwrap_or_default(),
            url: bmark_create.url,
            image_id: bmark_create.image_id,
            icon_id: bmark_create.icon_id,
        };

        self.bookmarks.write().unwrap().push(bookmark.clone());

        self.save();

        Ok(bookmark)
    }

    fn delete(&self, id: u64) -> anyhow::Result<Option<bool>> {
        let mut bmarks = self.bookmarks.write().unwrap();
        let result = bmarks.iter().position(|b| b.id == id).map(|idx| {
            bmarks.remove(idx);
            true
        });

        if result.is_some() {
            self.save();
        }

        Ok(result)
    }

    fn update(&self, id: u64, bmark_update: BookmarkUpdate) -> anyhow::Result<Option<Bookmark>> {
        let result = self
            .bookmarks
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
        let bmarks = self.bookmarks.read().unwrap();

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
                    let bookmark_url = bookmark.url.to_lowercase();
                    if !query.no_exact_url && bookmark_url == *url
                        || query.no_exact_url && bookmark_url.contains(url)
                    {
                        has_match = true;
                    } else {
                        return false;
                    }
                };

                if let Some(description) = &query.description {
                    let bookmark_description = bookmark.description.to_lowercase();
                    if query.exact && bookmark_description == *description
                        || !query.exact && bookmark_description.contains(description)
                    {
                        has_match = true;
                    } else {
                        return false;
                    }
                };

                if let Some(title) = &query.title {
                    let bookmark_title = bookmark.title.to_lowercase();
                    if query.exact && bookmark_title == *title
                        || !query.exact && bookmark_title.contains(title)
                    {
                        has_match = true;
                    } else {
                        return false;
                    }
                };

                if let Some(tags) = &query.tags {
                    let mut bookmark_tags = bookmark.tags.iter().map(|t| t.to_lowercase());
                    for tag in tags {
                        match bookmark_tags.find(|tag_b| tag == tag_b).is_none() {
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

// #[derive(Debug, Clone)]
// pub struct BookmarkMgrSqlite {
//     conn: Arc<RwLock<rusqlite::Connection>>,
// }
// impl BookmarkMgrSqlite {
//     pub fn conn() -> rustqlite::Connection {
//     }
//     pub fn new() -> Self {
//         let path = "./bookmarks.sqlite3";
//         let conn = rusqlite::Connection::open(path).unwrap();
//         {
//             let c = conn.write().unwrap();
//             c.execute(
//                 "CREATE TABLE IF NOT EXISTS bookmarks (
//                     id    INTEGER PRIMARY KEY AUTOINCREMENT,
//                     title TEXT NOT NULL,
//                     description TEXT NOT NULL,
//                     tags TEXT NOT NULL,
//                     url TEXT NOT NULL UNIQUE,
//                     image_id TEXT,
//                     icon_id TEXT,
//                 )",
//                 (), // empty list of parameters.
//             )
//             .unwrap();
//         }
//
//         Self { conn }
//     }
// }
//
// impl BookmarkMgrBackend for BookmarkMgrSqlite {
//     fn add(&self, bmark_create: BookmarkCreate) -> anyhow::Result<Bookmark> {
//         let conn = self.conn.write().unwrap();
//         conn.execute(
//             "INSERT INTO bookmarks (title, description, tags, url, image_id, icon_id) VALUES (?2, ?3, ?4, ?5, ?6, ?7)",
//             (&bmark_create.title.unwrap_or_default(), &bmark_create.description.unwrap_or_default(), &bmark_create.tags.unwrap_or_default().join(",")),
//         )?;
//         todo!()
//     }
// }
