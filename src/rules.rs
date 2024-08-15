use serde::{Deserialize, Serialize};

use crate::bookmarks::{Bookmark, BookmarkCreate, BookmarkUpdate, SearchQuery};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    pub action: Action,
}

impl Rule {
    // pub fn run_actions_on_bmark_insert(&self, mut bmark_create: BookmarkCreate) -> BookmarkCreate {
    //     // bookmark_mgr.update(id, bmark_update)
    //     match &self.action {
    //         Action::SetTitle(title) => bmark_create.title = Some(title.clone()),
    //         Action::SetDescription(descr) => bmark_create.description = Some(descr.clone()),
    //         Action::SetTags(tags) => bmark_create.tags = Some(tags.clone()),
    //         Action::AppendTags(tags) => {
    //             let mut tags = tags.clone();
    //             let mut current_tags = bmark_create.tags.take().unwrap_or_default();
    //             current_tags.append(&mut tags);
    //             bmark_create.tags.replace(current_tags);
    //         }
    //     }
    //
    //     bmark_create
    // }

    pub fn is_match(&self, query: &SearchQuery) -> bool {
        let mut matched = false;

        match (&self.url, &query.url) {
            (Some(match_url), Some(query_url)) => {
                matched = query_url.contains(match_url);
                // let regex = regex::Regex::new(&match_title).expect("incorrect regex for url match");
                // matched = regex.is_match(&query_title);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match (&self.title, &query.title) {
            (Some(match_title), Some(query_title)) => {
                matched = query_title.contains(match_title);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match (&self.description, &query.description) {
            (Some(match_description), Some(query_description)) => {
                matched = query_description.contains(match_description);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match &self.tags {
            Some(match_tags) => {
                // matching absence of tags
                let query_tags = &query.tags.clone().unwrap_or_default();

                if match_tags.is_empty() && query_tags.is_empty() {
                    return true;
                }

                let mut iter = query_tags.iter();

                for tag in match_tags.iter() {
                    if iter.find(|t| *t == tag).is_none() {
                        return false;
                    }
                }
            }
            _ => {}
        };

        return matched;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    UpdateBookmark {
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Vec<String>>,
    },
}
