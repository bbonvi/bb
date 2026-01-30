use regex::Regex;
use serde::{Deserialize, Serialize};

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
    pub query: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    pub action: Action,
}

#[derive(Debug, Clone, Default)]
pub struct Record {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl Rule {
    pub fn is_string_matches(match_query: &str, input: &str) -> bool {
        if match_query.starts_with("r/") && match_query.ends_with("/") {
            let mut match_query_chars = match_query.chars();

            // remove prefix and postfix
            match_query_chars.next();
            match_query_chars.next();
            match_query_chars.next_back();

            let regex = Regex::new(match_query_chars.as_str()).expect("malformed regex");
            regex.is_match(input)
        } else {
            input.to_lowercase().contains(&match_query.to_lowercase())
        }
    }

    pub fn is_match(&self, record: &Record) -> bool {
        let mut has_any_condition = false;

        if let Some(match_url) = &self.url {
            has_any_condition = true;
            if !Rule::is_string_matches(match_url, &record.url) {
                return false;
            }
        }

        if let (Some(match_title), Some(record_title)) = (&self.title, &record.title) {
            has_any_condition = true;
            if !Rule::is_string_matches(match_title, record_title) {
                return false;
            }
        }

        if let (Some(match_description), Some(record_description)) = (&self.description, &record.description) {
            has_any_condition = true;
            if !Rule::is_string_matches(match_description, record_description) {
                return false;
            }
        }

        if let Some(match_tags) = &self.tags {
            has_any_condition = true;
            let record_tags = record.tags.clone().unwrap_or_default();

            if match_tags.is_empty() {
                if !record_tags.is_empty() {
                    return false;
                }
            } else {
                let record_tags_lower: Vec<String> =
                    record_tags.iter().map(|t| t.to_lowercase()).collect();

                for tag in match_tags {
                    if !record_tags_lower.iter().any(|t| *t == tag.to_lowercase()) {
                        return false;
                    }
                }
            }
        }

        if let Some(query_str) = &self.query {
            has_any_condition = true;
            let temp_bookmark = crate::bookmarks::Bookmark {
                id: 0,
                url: record.url.clone(),
                title: record.title.clone().unwrap_or_default(),
                description: record.description.clone().unwrap_or_default(),
                tags: record.tags.clone().unwrap_or_default(),
                image_id: None,
                icon_id: None,
            };
            match crate::search_query::matches(query_str, &temp_bookmark) {
                Ok(true) => {}
                _ => return false,
            }
        }

        has_any_condition
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
