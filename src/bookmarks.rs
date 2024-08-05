use core::panic;
use std::{
    cell::RefCell,
    fs::{read_to_string, write},
    io::ErrorKind,
    sync::Arc,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: u64,

    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub url: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BookmarkShallow {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub url: String,
}

pub trait BookmarkBackend: Send + Sync {
    fn search(&self, query: Query) -> Vec<Bookmark>;
    fn add(&mut self, bookmark: BookmarkShallow) -> Vec<Bookmark>;
    fn delete(&mut self, id: u64) -> Option<bool>;
    fn update(&mut self, id: u64, shallow_bookmark: BookmarkShallow) -> Option<Bookmark>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bookmarks {
    pub bookmarks: Vec<Bookmark>,
}

impl Bookmarks {
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
        Self { bookmarks }
    }
    pub fn save(&self) {
        write(
            "bookmarks.json",
            serde_json::to_string(&self.bookmarks).unwrap(),
        )
        .unwrap();
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Query {
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

impl Query {
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

impl BookmarkBackend for Bookmarks {
    fn add(&mut self, shallow_bookmark: BookmarkShallow) -> Vec<Bookmark> {
        let id = if let Some(last_bookmark) = self.bookmarks.last() {
            last_bookmark.id + 1
        } else {
            0
        };

        let query = Query {
            url: Some(shallow_bookmark.url.clone()),
            ..Default::default()
        };

        if let Some(b) = self.search(query).first() {
            panic!(
                "bookmark with following url already exists at index {0}",
                b.id
            );
        };

        let bookmark = Bookmark {
            id,
            title: shallow_bookmark.title.unwrap_or_default(),
            description: shallow_bookmark.description.unwrap_or_default(),
            tags: shallow_bookmark.tags,
            url: shallow_bookmark.url,
        };

        self.bookmarks.push(bookmark.clone());

        self.save();

        vec![bookmark]
    }

    fn delete(&mut self, id: u64) -> Option<bool> {
        let result = self.bookmarks.iter().position(|b| b.id == id).map(|idx| {
            self.bookmarks.remove(idx);
            true
        });

        if result.is_some() {
            self.save();
        }

        result
    }

    fn update(&mut self, id: u64, shallow_bookmark: BookmarkShallow) -> Option<Bookmark> {
        let result = self.bookmarks.iter_mut().find(|b| b.id == id).map(|b| {
            b.title = shallow_bookmark.title.unwrap_or_default();
            b.description = shallow_bookmark.description.unwrap_or_default();
            b.tags = shallow_bookmark.tags;
            b.url = shallow_bookmark.url;

            b.clone()
        });

        if result.is_some() {
            self.save();
        }

        result
    }

    fn search(&self, query: Query) -> Vec<Bookmark> {
        let mut query = query;
        query.lowercase();

        self.bookmarks
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
            .collect::<Vec<_>>()
    }
}
