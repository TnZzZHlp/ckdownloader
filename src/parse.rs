use serde_json::Value;

use crate::{CLIENT, PB};

const PAGE_SIZE: usize = 50;

#[derive(serde::Deserialize, Debug, Clone)]
pub struct File {
    pub post_id: Option<String>,
    pub name: String,
    pub server: Option<String>,
    pub path: String,
}

pub async fn parse_artist_url(url: &str) -> anyhow::Result<Vec<File>> {
    let mut all_files = Vec::new();
    let mut offset = 0;
    let mut total_count: Option<usize> = None;

    let url = reqwest::Url::parse(url)?;

    let domain = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid URL:{}", url))?;
    let parts = url.path_segments().unwrap().collect::<Vec<_>>().join("/");

    loop {
        let _ = PB.println(format!(
            "Fetching data for page {}...",
            offset / PAGE_SIZE + 1
        ));
        let api_url = format!("https://{domain}/api/v1/{parts}/posts-legacy?o={offset}");
        let resp = CLIENT.get().unwrap().get(&api_url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Unable to access {}", api_url);
        }
        let data = resp.json::<serde_json::Value>().await?;
        if total_count.is_none()
            && let Some(props) = data.get("props")
        {
            total_count = Some(props.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
        }

        if let Some(results) = data.get("results") {
            let results: Vec<Value> = serde_json::from_value(results.clone())?;
            results.iter().for_each(|result| {
                if let Some(file) = result.get("file")
                    && let Ok(mut file) = serde_json::from_value::<File>(file.clone())
                {
                    file.post_id = result
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    all_files.push(file);
                }

                if let Some(thumb) = result.get("attachments")
                    && let Ok(files) = serde_json::from_value::<Vec<File>>(thumb.clone())
                {
                    files.into_iter().for_each(|mut file| {
                        file.post_id = result
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        all_files.push(file);
                    });
                }
            });
        }

        offset += PAGE_SIZE;

        // Judge if we have fetched all pages
        if let Some(count) = total_count
            && offset >= count
        {
            break;
        }
    }

    Ok(all_files)
}
