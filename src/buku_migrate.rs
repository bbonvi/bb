use crate::{bookmarks::BookmarkManager, eid::Eid, storage::StorageManager};

pub fn migrate() {
    let buku_str = std::fs::read_to_string("./buku.json").expect("buku.json not found");
    let buku: serde_json::Value = serde_json::from_str(&buku_str).expect("couldnt parse buku.json");

    let bmarks = crate::bookmarks::BackendCsv::load("bookmarks-from-buku.csv").unwrap();

    let storage_mgr = crate::storage::BackendLocal::new("./uploads");

    match buku {
        serde_json::Value::Array(buku_array) => {
            for record in buku_array {
                let description =
                    if let serde_json::Value::String(s) = record.get("description").unwrap() {
                        s.clone()
                    } else {
                        panic!()
                    };
                let title = if let serde_json::Value::String(s) = record.get("title").unwrap() {
                    s.clone()
                } else {
                    panic!()
                };
                let url = if let serde_json::Value::String(s) = record.get("url").unwrap() {
                    s.clone()
                } else {
                    panic!()
                };
                let tags = match record.get("tags").unwrap() {
                    serde_json::Value::Array(tags_array) => tags_array,
                    _ => unreachable!(),
                };

                let mut icon_id = None;
                let mut image_id = None;
                let http_client = reqwest::blocking::Client::new();

                {
                    let url_parsed = reqwest::Url::parse(&url).unwrap();
                    let host = url_parsed.host_str();

                    if let Some(host) = host {
                        let icon_url =
                            format!("https://external-content.duckduckgo.com/ip3/{host}.ico");
                        if let Ok(resp) = http_client.get(&icon_url).send() {
                            if resp.status().as_u16() == 200 {
                                let file = resp.bytes().unwrap();

                                let filetype = infer::get(&file)
                                    .map(|ftype| ftype.extension())
                                    .unwrap_or("png")
                                    .to_string();

                                icon_id = Some(format!("{}.{}", Eid::new(), filetype));
                                storage_mgr.write(&icon_id.clone().unwrap(), &file);
                            }
                        };
                    }
                }

                log::info!("{}", format!("http://buku.localhost/cached/{}", url));
                if let Ok(resp) = http_client
                    .get(format!("http://buku.localhost/cached/{}", url))
                    .basic_auth("bn", Some("cleanBeaverZ"))
                    .send()
                {
                    if resp.status().as_u16() >= 400
                        && resp.status().as_u16() != 400
                        && resp.status().as_u16() != 404
                    {
                        log::error!("{resp:?}");
                    }

                    let file = resp.bytes().unwrap();
                    let filetype = infer::get(&file)
                        .map(|ftype| ftype.extension())
                        .unwrap_or("png")
                        .to_string();

                    image_id = Some(format!("{}.{}", Eid::new(), filetype));
                    storage_mgr.write(&image_id.clone().unwrap(), &file);
                };

                bmarks
                    .create(crate::bookmarks::BookmarkCreate {
                        title: Some(title.to_string()),
                        description: Some(description.to_string()),
                        tags: Some(
                            tags.into_iter()
                                .map(|t| {
                                    if let serde_json::Value::String(t) = t {
                                        t.clone()
                                    } else {
                                        panic!()
                                    }
                                })
                                .filter(|t| !t.contains(" "))
                                .collect::<Vec<_>>()
                                .clone(),
                        ),
                        url: url.to_string(),
                        image_id: image_id,
                        icon_id: icon_id,
                    })
                    .unwrap();
            }
        }
        _ => unreachable!(),
    }
}
