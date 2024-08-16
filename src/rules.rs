use serde::{Deserialize, Serialize};

use crate::bookmarks::SearchQuery;

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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    pub action: Action,
}

impl Rule {
    pub fn is_string_matches(match_query: &str, input: &str) -> bool {
        if match_query.starts_with("r/") && match_query.ends_with("/") {
            let mut match_query_chars = match_query.chars();

            // remove prefix and postfix
            match_query_chars.next();
            match_query_chars.next();
            match_query_chars.next_back();

            let regex = regex::Regex::new(&match_query_chars.as_str()).expect("malformed regex");
            regex.is_match(&input)
        } else {
            match_query.to_lowercase().contains(&input)
        }
    }

    pub fn is_match(&self, bookmark_match: &SearchQuery) -> bool {
        let mut matched = false;

        match (&self.url, &bookmark_match.url) {
            (Some(match_url), Some(bookmark_url)) => {
                matched = Rule::is_string_matches(match_url, bookmark_url);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match (&self.title, &bookmark_match.title) {
            (Some(match_title), Some(bookmark_title)) => {
                matched = Rule::is_string_matches(match_title, bookmark_title);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match (&self.description, &bookmark_match.description) {
            (Some(match_description), Some(bookmark_description)) => {
                matched = Rule::is_string_matches(match_description, bookmark_description);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match &self.tags {
            Some(match_tags) => {
                // matching absence of tags
                let bookmark_tags = &bookmark_match.tags.clone().unwrap_or_default();

                if match_tags.is_empty() && bookmark_tags.is_empty() {
                    return true;
                }

                let mut iter = bookmark_tags.iter();

                for tag in match_tags.iter() {
                    if iter
                        .find(|t| *t.to_lowercase() == tag.to_lowercase())
                        .is_none()
                    {
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
