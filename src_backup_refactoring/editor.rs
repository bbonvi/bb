use crate::eid::Eid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EditorValue<T> {
    Set(T),
    Unset,
    Ignore,
}

impl<T> Default for EditorValue<T> {
    fn default() -> Self {
        EditorValue::Ignore
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct EditorBookmark {
    pub url: String,
    pub title: EditorValue<String>,
    pub tags: EditorValue<String>,
    pub description: EditorValue<String>,
}

fn parse_editor_bookmark(input: &str) -> anyhow::Result<EditorBookmark> {
    enum CurrLine {
        Url,
        Title,
        Tags,
        Description,
        None,
    }

    let mut curr_line = CurrLine::None;

    let mut url = String::new();
    let mut title = String::new();
    let mut tags = String::new();
    let mut description = String::new();

    for line in input.lines() {
        let line = line.trim().to_string();

        if line.starts_with("# URL") {
            curr_line = CurrLine::Url;
            continue;
        }

        if line.starts_with("# TITLE") {
            curr_line = CurrLine::Title;
            continue;
        }

        if line.starts_with("# TAGS") {
            curr_line = CurrLine::Tags;
            continue;
        }

        if line.starts_with("# DESCRIPTION") {
            curr_line = CurrLine::Description;
            continue;
        }

        if line.starts_with("# CURRENT TAGS FOR REFERENCE") {
            break;
        }

        if line.is_empty() {
            if let CurrLine::Description = curr_line {
                description.push_str("\n");
                description.push_str(&line);
            }

            continue;
        }

        match curr_line {
            CurrLine::Url => {
                url = line.clone();
                curr_line = CurrLine::None;
            }
            CurrLine::Title => {
                title = line.clone();
                curr_line = CurrLine::None;
            }
            CurrLine::Tags => {
                tags = line.clone();
                curr_line = CurrLine::None;
            }
            CurrLine::Description => {
                description.push_str("\n");
                description.push_str(&line);
            }
            CurrLine::None => {}
        };
    }

    url = url.trim().to_string();
    title = title.trim().to_string();
    description = description.trim().to_string();
    tags = tags.trim().to_string();

    if url.is_empty() {
        anyhow::bail!("url cannot be empty!")
    }

    let mut editor_bookmark = EditorBookmark::default();

    editor_bookmark.url = url;

    if !title.is_empty() {
        if &title == "-" {
            editor_bookmark.title = EditorValue::Unset;
        } else {
            editor_bookmark.title = EditorValue::Set(title);
        }
    }

    if !tags.is_empty() {
        if &tags == "-" {
            editor_bookmark.tags = EditorValue::Unset;
        } else {
            editor_bookmark.tags = EditorValue::Set(tags);
        }
    }

    if !description.is_empty() {
        if &description == "-" {
            editor_bookmark.description = EditorValue::Unset;
        } else {
            editor_bookmark.description = EditorValue::Set(description);
        }
    }

    Ok(editor_bookmark)
}

#[derive(Default, Debug)]
pub struct EditorDefaults {
    pub url: Option<String>,
    pub tags: Option<String>,
    pub description: Option<String>,
    pub title: Option<String>,
    pub current_tags: Vec<String>,
}

pub fn edit(opts: EditorDefaults) -> anyhow::Result<EditorBookmark> {
    let editor = std::env::var("EDITOR").unwrap_or("vim".into());

    let temp_file = format!("/tmp/bb-{}.md", Eid::new());
    std::fs::write(
        &temp_file,
        format!(
            r###"# URL (one line):
{}
# TITLE (one line, leave "-" to prevent auto-fill)
{}
# TAGS (one line, comma/space separated, leave "-" to prevent auto-fill)
{}
# DESCRIPTION (multi-line, leave "-" to prevent auto-fill)
{}










# CURRENT TAGS FOR REFERENCE AND AUTOCOMPLETION (do not change this line)
{}
"###,
            opts.url.unwrap_or_default(),
            opts.title.unwrap_or_default(),
            opts.tags.unwrap_or_default(),
            opts.description.unwrap_or_default(),
            opts.current_tags.join(" ")
        ),
    )
    .expect("error writing temp file");

    let shell = std::env::var("SHELL").unwrap_or("/usr/sbin/bash".into());
    std::process::Command::new(shell)
        .arg("-c")
        .arg(format!("{editor} {temp_file}"))
        .spawn()
        .expect("Error: Failed to run editor")
        .wait()
        .expect("Error: Editor returned a non-zero status");

    let content = std::fs::read_to_string(&temp_file).expect("error reading temp file");

    std::fs::remove_file(&temp_file).expect("error deleting temp file");

    parse_editor_bookmark(&content)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn test_editor_parse_1() {
        let input = r###"
        # URL (one line):
        http://example.com/lmao

        # TITLE (one line, leave "-" to prevent auto-fill)
        title

        # TAGS (one line, comma/space separated, leave "-" to prevent auto-fill)
        tag1,tag2

        # DESCRIPTION (multi-line, leave "-" to prevent auto-fill)
        dummy
        "###;

        let result = EditorBookmark {
            url: "http://example.com/lmao".to_string(),
            title: EditorValue::Set("title".into()),
            tags: EditorValue::Set("tag1,tag2".into()),
            description: EditorValue::Set("dummy".into()),
        };

        assert_eq!(result, parse_editor_bookmark(input).unwrap());
    }

    #[test]
    pub fn test_editor_parse_2() {
        let input = r###"
        # URL (one line):
        http://example.com/lmao

        # TITLE (one line, leave "-" to prevent auto-fill)

        # TAGS (one line, comma/space separated, leave "-" to prevent auto-fill)
        -

        # DESCRIPTION (multi-line, leave "-" to prevent auto-fill)
        multiline
        description

        over
        here
        # CURRENT TAGS FOR REFERENCE AND AUTOCOMPLETION (do not change this line)
        "###;

        let result = EditorBookmark {
            url: "http://example.com/lmao".to_string(),
            title: EditorValue::Ignore,
            tags: EditorValue::Unset,
            description: EditorValue::Set("multiline\ndescription\n\nover\nhere".into()),
        };

        assert_eq!(result, parse_editor_bookmark(input).unwrap());
    }
}
