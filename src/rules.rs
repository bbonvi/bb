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

            let regex = regex::Regex::new(&match_query_chars.as_str()).expect("malformed regex");
            regex.is_match(&input)
        } else {
            input.to_lowercase().contains(&match_query.to_lowercase())
        }
    }

    pub fn is_match(&self, record: &Record) -> bool {
        let mut matched = false;

        match &self.url {
            Some(match_url) => {
                matched = Rule::is_string_matches(match_url, &record.url);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match (&self.title, &record.title) {
            (Some(match_title), Some(record_title)) => {
                matched = Rule::is_string_matches(match_title, record_title);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match (&self.description, &record.description) {
            (Some(match_description), Some(record_description)) => {
                matched = Rule::is_string_matches(match_description, record_description);
                if !matched {
                    return false;
                }
            }
            _ => {}
        };

        match &self.tags {
            Some(match_tags) => {
                // matching absence of tags
                let record_tags = &record.tags.clone().unwrap_or_default();

                if match_tags.is_empty() && record_tags.is_empty() {
                    return true;
                }

                let mut iter = record_tags.iter();

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
