use crate::parse_tags;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    hash::Hash,
    io::ErrorKind,
    sync::{Arc, RwLock},
    time::Instant,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub append_tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove_tags: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SearchQuery {
    pub id: Option<u64>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub keyword: Option<String>,

    /// Semantic search query text (not lowercased—preserves embedding intent)
    #[serde(default)]
    pub semantic: Option<String>,

    /// Similarity threshold for semantic search [0.0, 1.0]
    #[serde(default)]
    pub threshold: Option<f32>,

    #[serde(default)]
    pub exact: bool,

    #[serde(default)]
    pub limit: Option<usize>,
}

pub trait BookmarkManager: Send + Sync {
    fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<Bookmark>>;
    fn search_update(&self, query: SearchQuery, update: BookmarkUpdate) -> anyhow::Result<usize>;
    fn search_delete(&self, query: SearchQuery) -> anyhow::Result<usize>;
    fn create(&self, bookmark: BookmarkCreate) -> anyhow::Result<Bookmark>;
    fn update(&self, id: u64, update: BookmarkUpdate) -> anyhow::Result<Bookmark>;
    fn delete(&self, id: u64) -> anyhow::Result<()>;
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
        self.keyword = self.keyword.as_ref().map(|keyword| keyword.to_lowercase());
    }
}

#[derive(Debug, Clone, Default)]
pub struct BackendCsv {
    list: Arc<RwLock<Vec<Bookmark>>>,
    path: String,
}

const CSV_HEADERS: [&str; 7] = [
    "id",
    "url",
    "title",
    "description",
    "tags",
    "image_id",
    "icon_id",
];

impl BackendCsv {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        if let Err(err) = std::fs::metadata(path) {
            match err.kind() {
                ErrorKind::NotFound => {
                    log::info!("Creating new database at {path}");
                    let mut csv_wrt = csv::Writer::from_path(path)?;
                    csv_wrt.write_record(CSV_HEADERS)?;
                    csv_wrt.flush()?;
                }
                _ => Err(err)?,
            }
        }

        let now = Instant::now();
        let mut csv_reader = csv::Reader::from_path(path)?;
        let iter = csv_reader.records();

        let mut bmarks = vec![];
        for record in iter {
            let record = record?;
            let id = record
                .get(0)
                .ok_or(anyhow!("couldnt get record id"))?
                .parse::<u64>()?;
            let url = record
                .get(1)
                .ok_or(anyhow!("couldnt get record url"))?
                .to_string();
            let title = record
                .get(2)
                .ok_or(anyhow!("couldnt get record title"))?
                .to_string();
            let description = record
                .get(3)
                .ok_or(anyhow!("couldnt get record description"))?
                .to_string();
            let tags = parse_tags(
                record
                    .get(4)
                    .ok_or(anyhow!("couldnt get record tags"))?
                    .to_string(),
            );
            let image_id = record
                .get(5)
                .ok_or(anyhow!("couldnt get record tags"))?
                .to_string();
            let icon_id = record
                .get(6)
                .ok_or(anyhow!("couldnt get record tags"))?
                .to_string();

            let bmark = Bookmark {
                id,
                title,
                description,
                tags,
                url,
                image_id: if image_id.is_empty() {
                    None
                } else {
                    Some(image_id)
                },
                icon_id: if icon_id.is_empty() {
                    None
                } else {
                    Some(icon_id)
                },
            };

            bmarks.push(bmark);
        }

        log::debug!(
            "took {}ms to read csv",
            now.elapsed().as_micros() as f64 / 1000.0
        );

        let mgr = BackendCsv {
            list: Arc::new(RwLock::new(bmarks)),
            path: path.to_string(),
        };

        Ok(mgr)
    }

    pub fn save(&self) {
        let bmarks = self.list.write().unwrap();

        let temp_path = format!("{}-tmp", &self.path);
        let mut csv_wrt = csv::Writer::from_path(&temp_path).unwrap();
        csv_wrt.write_record(CSV_HEADERS).unwrap();
        for bmark in bmarks.iter() {
            csv_wrt
                .write_record([
                    &bmark.id.to_string(),
                    &bmark.url,
                    &bmark.title,
                    &bmark.description,
                    &bmark.tags.join(","),
                    &bmark.image_id.clone().unwrap_or_default(),
                    &bmark.icon_id.clone().unwrap_or_default(),
                ])
                .unwrap();
        }
        csv_wrt.flush().unwrap();
        std::fs::rename(&temp_path, &self.path).unwrap();
    }

    #[cfg(test)]
    pub fn wipe_database(self) -> Self {
        let _ = std::fs::remove_file(&self.path);
        *self.list.write().unwrap() = vec![];
        self
    }
}

impl BookmarkManager for BackendCsv {
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

    fn delete(&self, id: u64) -> anyhow::Result<()> {
        let mut bmarks = self.list.write().unwrap();
        let result = bmarks.iter().position(|b| b.id == id).map(|idx| {
            bmarks.remove(idx);
            true
        });

        drop(bmarks);

        if result.is_some() {
            self.save();
        }

        Ok(())
    }

    fn update(&self, id: u64, bmark_update: BookmarkUpdate) -> anyhow::Result<Bookmark> {
        let mut bmarks = self.list.write().unwrap();

        let bmark_idx = bmarks
            .iter()
            .position(|b| b.id == id)
            .ok_or_else(|| anyhow::anyhow!("Bookmark with id {} not found", id))?;

        let bmark = &mut bmarks[bmark_idx];

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

        if let Some(delete_tags) = bmark_update.remove_tags {
            bmark
                .tags
                .retain(|item| !delete_tags.iter().any(|t| t == item));
        }

        if let Some(mut tags) = bmark_update.append_tags {
            bmark.tags.append(&mut tags);
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

        let result = bmark.clone();
        drop(bmarks);

        self.save();

        Ok(result)
    }

    fn search_delete(&self, query: SearchQuery) -> anyhow::Result<usize> {
        let results = self.search(query)?;
        let delete_ids = results;
        let count = delete_ids.len();

        let mut bmarks = self.list.write().unwrap();
        *bmarks = bmarks
            .iter()
            .filter(|b| !delete_ids.iter().any(|bb| b.id == bb.id))
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
            if !results.iter().any(|b| b.id == bmark.id) {
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

            if let Some(ref delete_tags) = bmark_update.remove_tags {
                bmark
                    .tags
                    .retain(|item| !delete_tags.iter().any(|t| t == item));
            }

            if let Some(ref tags) = bmark_update.append_tags {
                let mut t = tags.clone();
                bmark.tags.append(&mut t);
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
            && query.keyword.is_none()
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
                if !tags.is_empty() {
                    for (tag, teg_delim, neg_tag, neg_tag_delim) in tags {
                        let mut bmark_tags = bmark_tags.iter();
                        if let Some(neg_tag) = neg_tag {
                            if bmark_tags
                                .any(|tag_b| neg_tag == tag_b || tag_b.contains(neg_tag_delim))
                            {
                                has_match = false;
                                break;
                            } else {
                                has_match = true;
                            }
                        } else if !bmark_tags.any(|tag_b| tag == tag_b || tag_b.contains(teg_delim))
                        {
                            has_match = false;
                            break;
                        } else {
                            has_match = true;
                        }
                    }

                    if !has_match {
                        continue;
                    }
                }
            };

            // Keyword search - matches across title, description, url, and tags
            if let Some(keyword) = &query.keyword {
                // For non-exact mode, split by whitespace and check each keyword
                let keywords: Vec<&str> = keyword.split_whitespace().collect();

                // A bookmark matches if ALL keywords are found in ANY field
                let mut keywords_match = true;

                for keyword in keywords {
                    // Check if this keyword starts with # for tag search
                    if bmark_tags.iter().any(|tag| tag.contains(keyword)) {
                        continue;
                    }

                    // Check title
                    if bookmark.title.to_lowercase().contains(keyword) {
                        continue;
                    }

                    // Check description
                    if bookmark.description.to_lowercase().contains(keyword) {
                        continue;
                    }

                    // Check URL
                    if bookmark.url.to_lowercase().contains(keyword) {
                        continue;
                    }

                    // If we reach here, the keyword was not found in any field
                    keywords_match = false;
                    break;
                }

                if !keywords_match {
                    continue;
                } else {
                    has_match = true;
                }
            };

            if has_match {
                output.push(bookmark.clone());
            }

            let id_query = query.id.is_some();
            let limit_reached =
                query.limit.is_some() && output.len() >= query.limit.unwrap_or_default();

            if id_query || limit_reached {
                break;
            }
        }

        Ok(output)
    }
}

#[cfg(test)]
impl BackendCsv {
    #[cfg(test)]
    pub fn list(&self) -> Arc<RwLock<Vec<Bookmark>>> {
        self.list.clone()
    }
}
