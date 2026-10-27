//! Public artwork lookup, independent of launch integration.

use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct SearchResults {
    items: Vec<StoreItem>,
}

#[derive(Deserialize)]
struct StoreItem {
    id: u32,
    name: String,
    #[serde(rename = "type")]
    kind: String,
}

fn normalized_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn matching_app(items: &[StoreItem], name: &str) -> Option<u32> {
    let name = normalized_name(match normalized_name(name).as_str() {
        "nte" | "nevernesstoeverness" => "NTE: Neverness to Everness",
        "arknightendfield" => "Arknights: Endfield",
        _ => name,
    });
    let mut matches = items
        .iter()
        .filter(|item| item.id > 0 && item.kind == "app" && normalized_name(&item.name) == name);
    let first = matches.next()?;
    matches.next().is_none().then_some(first.id)
}

fn epic_artwork(data: &serde_json::Value, name: &str) -> Vec<String> {
    let Some(items) = data["elements"].as_array() else {
        return Vec::new();
    };
    let mut matches = items.iter().filter(|item| {
        item["offerType"] == "BASE_GAME"
            && item["title"]
                .as_str()
                .is_some_and(|title| normalized_name(title) == normalized_name(name))
    });
    let Some(item) = matches.next() else {
        return Vec::new();
    };
    if matches.next().is_some() {
        return Vec::new();
    }
    item["keyImages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|image| image["type"] == "OfferImageWide")
        .filter_map(|image| {
            let raw = image["url"].as_str()?;
            let url = reqwest::Url::parse(raw).ok()?;
            (url.scheme() == "https"
                && matches!(
                    url.host_str(),
                    Some("cdn1.epicgames.com") | Some("cdn2.unrealengine.com")
                ))
            .then(|| raw.to_string())
        })
        .collect()
}

fn resolve_epic_artwork(
    client: &reqwest::blocking::Client,
    name: &str,
) -> Result<Vec<String>, String> {
    let mut url = reqwest::Url::parse("https://api.egdata.app/autocomplete")
        .map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .extend_pairs([("query", name), ("limit", "20"), ("locale", "en-US")]);
    let data = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json())
        .map_err(|error| error.to_string())?;
    Ok(epic_artwork(&data, name))
}

fn image_url(value: &serde_json::Value) -> Option<String> {
    let raw = value.as_str()?;
    let url = reqwest::Url::parse(raw).ok()?;
    (url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("shared.akamai.steamstatic.com")
                | Some("shared.fastly.steamstatic.com")
                | Some("cdn.akamai.steamstatic.com")
        ))
    .then(|| raw.to_string())
}

fn find_steam_app(client: &reqwest::blocking::Client, name: &str) -> Result<Option<u32>, String> {
    let mut url = reqwest::Url::parse("https://store.steampowered.com/api/storesearch/")
        .map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .extend_pairs([("term", name), ("l", "english"), ("cc", "us")]);
    let results: SearchResults = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json())
        .map_err(|error| error.to_string())?;
    Ok(matching_app(&results.items, name))
}

fn steam_cover_urls(assets: &serde_json::Value, id: u32) -> Vec<String> {
    let format = assets["asset_url_format"].as_str().unwrap_or("");
    let prefix = format!("steam/apps/{id}/");
    if !format.starts_with(&prefix) || format.matches("${FILENAME}").count() != 1 {
        return Vec::new();
    }
    ["library_capsule_2x", "library_capsule"]
        .iter()
        .filter_map(|key| {
            let path = assets[*key].as_str()?;
            if path.is_empty()
                || path
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
                || !path
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_/ .".contains(c))
            {
                return None;
            }
            let url = format!(
                "https://shared.fastly.steamstatic.com/store_item_assets/{}",
                format.replace("${FILENAME}", path)
            );
            image_url(&serde_json::Value::String(url))
        })
        .collect()
}

pub(crate) fn resolve_steam_cover(
    name: String,
    steam_app_id: Option<String>,
) -> Result<Vec<String>, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 200 {
        return Err("Invalid game name".into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("Dummies/1.0")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?;
    let id = match steam_app_id
        .as_deref()
        .and_then(|id| id.trim().parse::<u32>().ok())
        .filter(|id| *id > 0)
    {
        Some(id) => id,
        None => match find_steam_app(&client, name)? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        },
    };
    let input = serde_json::json!({"ids":[{"appid":id}], "context":{"language":"english","country_code":"US","steam_realm":1}, "data_request":{"include_assets":true}});
    let mut url =
        reqwest::Url::parse("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/")
            .map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .append_pair("input_json", &input.to_string());
    let data: serde_json::Value = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json())
        .map_err(|error| error.to_string())?;
    let item = &data["response"]["store_items"][0];
    if item["appid"].as_u64() != Some(u64::from(id)) {
        return Ok(Vec::new());
    }
    Ok(steam_cover_urls(&item["assets"], id))
}

pub(crate) fn resolve_steam_artwork(
    name: String,
    steam_app_id: Option<String>,
) -> Result<Vec<String>, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 200 {
        return Err("Invalid game name".into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("Dummies/1.0")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?;
    let app_id = steam_app_id
        .as_deref()
        .and_then(|id| id.trim().parse::<u32>().ok())
        .filter(|id| *id > 0);
    let steam = || -> Result<Vec<String>, String> {
        let app_id = match app_id {
            Some(id) => id,
            None => {
                let Some(id) = find_steam_app(&client, name)? else {
                    return Ok(Vec::new());
                };
                id
            }
        };
        let details: serde_json::Value = client
            .get(format!(
                "https://store.steampowered.com/api/appdetails?appids={app_id}&l=english&cc=us"
            ))
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.json())
            .map_err(|error| error.to_string())?;
        let entry = &details[app_id.to_string()];
        if entry["success"] != true {
            return Ok(Vec::new());
        }
        let data = &entry["data"];
        Ok(["background_raw", "header_image"]
            .iter()
            .filter_map(|field| image_url(&data[field]))
            .collect())
    };
    let mut urls = steam().unwrap_or_default();
    urls.extend(resolve_epic_artwork(&client, name).unwrap_or_default());
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matches_names_without_guessing_similar_games_or_packages() {
        let items: Vec<StoreItem> = serde_json::from_value(json!([
            {"id": 1, "name": "Arknights: Endfield Demo", "type": "app"},
            {"id": 4732690, "name": "Arknights: Endfield", "type": "app"},
            {"id": 4126040, "name": "Aniimo", "type": "app"},
            {"id": 3393110, "name": "AION 2", "type": "app"},
            {"id": 4508340, "name": "NTE: Neverness to Everness", "type": "app"},
            {"id": 2, "name": "Unknown", "type": "sub"}
        ]))
        .unwrap();
        assert_eq!(matching_app(&items, "ARKNIGHTS ENDFIELD"), Some(4732690));
        assert_eq!(matching_app(&items, "aniimo"), Some(4126040));
        assert_eq!(matching_app(&items, "AION2"), Some(3393110));
        assert_eq!(matching_app(&items, "Unknown"), None);
        assert_eq!(matching_app(&items, "Arknights"), None);
        assert_eq!(matching_app(&items, "Arknight Endfield"), Some(4732690));
        assert_eq!(matching_app(&items, "NTE"), Some(4508340));
        assert_eq!(matching_app(&items, ""), None);
        assert_eq!(matching_app(&items[..1], "Arknights Endfield"), None);
        let duplicates = serde_json::from_value::<Vec<StoreItem>>(json!([
            {"id": 1, "name": "Aniimo", "type": "app"},
            {"id": 2, "name": "Aniimo", "type": "app"}
        ]))
        .unwrap();
        assert_eq!(matching_app(&duplicates, "Aniimo"), None);
    }

    #[test]
    fn epic_requires_an_exact_base_game_and_a_trusted_wide_image() {
        let data = json!({"elements": [
            {"title":"Genshin Impact", "offerType":"DLC", "keyImages":[{"type":"OfferImageWide", "url":"https://cdn1.epicgames.com/wrong"}]},
            {"title":"Genshin Impact", "offerType":"BASE_GAME", "keyImages":[
                {"type":"OfferImageTall", "url":"https://cdn1.epicgames.com/portrait"},
                {"type":"OfferImageWide", "url":"https://example.com/wrong"},
                {"type":"OfferImageWide", "url":"https://cdn1.epicgames.com/hero"}
            ]}
        ]});
        assert_eq!(
            epic_artwork(&data, "genshin impact"),
            vec!["https://cdn1.epicgames.com/hero"]
        );
        assert!(epic_artwork(&data, "genshin").is_empty());
    }

    #[test]
    fn cover_urls_use_current_steam_assets_and_reject_bad_paths() {
        let assets = json!({"asset_url_format":"steam/apps/4732690/${FILENAME}?t=1", "library_capsule_2x":"abc/library_capsule_2x.jpg", "library_capsule":"../bad.jpg"});
        assert_eq!(
            steam_cover_urls(&assets, 4732690),
            vec![
                "https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/4732690/abc/library_capsule_2x.jpg?t=1"
            ]
        );
        assert!(steam_cover_urls(&assets, 730).is_empty());
    }

    #[test]
    fn accepts_current_hashed_images_only_from_allowed_https_hosts() {
        let url = "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/4732690/hash/page_bg_raw.jpg?t=123";
        assert_eq!(image_url(&json!(url)).as_deref(), Some(url));
        assert_eq!(image_url(&json!("https://example.com/image.jpg")), None);
        assert_eq!(
            image_url(&json!("http://shared.akamai.steamstatic.com/image.jpg")),
            None
        );
        assert_eq!(image_url(&json!(null)), None);
    }

    #[test]
    #[ignore = "requires access to live artwork services"]
    fn live_steam_covers_for_recent_releases() {
        let client = reqwest::blocking::Client::new();
        for name in ["ARKNIGHTS: ENDFIELD", "Aniimo", "NTE", "Zenless Zone Zero"] {
            let urls = resolve_steam_cover(name.into(), None).unwrap();
            assert!(!urls.is_empty(), "No Steam cover for {name}");
            client
                .get(&urls[0])
                .send()
                .unwrap()
                .error_for_status()
                .unwrap();
        }
    }

    #[test]
    #[ignore = "requires access to live artwork services"]
    fn live_steam_artwork_for_recent_releases() {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        for (name, id) in [
            ("ARKNIGHTS: ENDFIELD", "4732690"),
            ("Aniimo", "4126040"),
            ("AION 2", "3393110"),
            ("NTE", "4508340"),
            ("Zenless Zone Zero", "4162040"),
        ] {
            let urls = resolve_steam_artwork(name.into(), None).unwrap();
            assert!(!urls.is_empty(), "No artwork found for {name}");
            let steam_urls: Vec<_> = urls
                .iter()
                .filter(|url| url.contains(&format!("/apps/{id}/")))
                .collect();
            assert!(!steam_urls.is_empty(), "No Steam artwork for {name}");
            for url in steam_urls {
                client.get(url).send().unwrap().error_for_status().unwrap();
            }
        }
        let urls = resolve_epic_artwork(&client, "Genshin Impact").unwrap();
        assert!(!urls.is_empty(), "No Epic artwork for Genshin Impact");
        client
            .get(&urls[0])
            .send()
            .unwrap()
            .error_for_status()
            .unwrap();
    }
}
