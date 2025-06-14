use std::{sync::Arc, time::Duration};

use futures_util::lock::Mutex;
use indicatif::{ProgressBar, style};
use tokio::task::JoinSet;

use crate::{CLIENT, PB};

const PAGE_SIZE: usize = 50;
pub async fn parse_artist_url(url: &str) -> anyhow::Result<Vec<String>> {
    let mut all_ids = Vec::new();
    let mut offset = 0;
    let mut total_count: Option<usize> = None;

    let url = reqwest::Url::parse(url)?;

    let domain = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("无效的 URL: {}", url))?;
    let parts = url.path_segments().unwrap().collect::<Vec<_>>().join("/");

    loop {
        let _ = PB.println(format!("正在获取第 {} 页数据...", offset / PAGE_SIZE + 1));
        let api_url = format!("https://{domain}/api/v1/{parts}/posts-legacy?o={offset}");
        let resp = CLIENT.get(&api_url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("无法访问 {}", api_url);
        }
        let data = resp.json::<serde_json::Value>().await?;
        if total_count.is_none() {
            if let Some(props) = data.get("props") {
                total_count =
                    Some(props.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
            }
        }
        let ids: Vec<_> = data
            .get("results")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new())
            .iter()
            .map(|x| {
                x.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0")
                    .to_string()
            })
            .collect();
        if ids.is_empty() {
            break;
        }
        all_ids.extend(ids);
        if total_count.is_some_and(|tc| all_ids.len() >= tc) {
            break;
        }
        offset += PAGE_SIZE;
    }
    Ok(all_ids)
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Attachment {
    pub name: String,
    pub server: Option<String>,
    pub path: String,
}

pub async fn get_details(url: &str, ids: Vec<String>) -> anyhow::Result<Vec<Attachment>> {
    let url = reqwest::Url::parse(url)?;
    let domain = Arc::new(
        url.host_str()
            .ok_or_else(|| anyhow::anyhow!("无效的 URL: {}", url))?
            .to_owned(),
    );
    let parts = Arc::new(url.path_segments().unwrap().collect::<Vec<_>>().join("/"));

    let attachments = Arc::new(Mutex::new(Vec::new()));
    let mut tasks = JoinSet::new();

    let pb = Arc::new(PB.add(ProgressBar::new(ids.len() as u64)));
    pb.set_message("正在获取详情...");
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_style(
        style::ProgressStyle::with_template(
            "[{elapsed_precise}] [{wide_bar:.cyan/blue}] [{pos}/{len}]",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    for id in ids {
        let attachments = Arc::clone(&attachments);
        let domain = Arc::clone(&domain);
        let parts = Arc::clone(&parts);
        let pb = Arc::clone(&pb);
        let sem = crate::SEM.clone();
        tasks.spawn(async move {
            let _permit = sem.acquire().await;
            let mut attachments = attachments.lock().await;
            let url = format!("https://{domain}/api/v1/{parts}/post/{id}");
            let resp = CLIENT.get(&url).send().await;
            match resp {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp.json().await.unwrap_or_default();
                    if let Some(atts) = json.get("attachments") {
                        let list: Vec<Attachment> =
                            serde_json::from_value(atts.clone()).unwrap_or_default();
                        attachments.extend(list);
                    }
                }
                _ => {
                    let _ = PB.println(format!("获取详情失败 {}", id));
                }
            }
            pb.inc(1);
        });
    }

    tasks.join_all().await;

    let _ = PB.println(format!(
        "获取详情完成，共 {} 个资源",
        attachments.lock().await.len()
    ));

    pb.finish_and_clear();

    Ok(Arc::try_unwrap(attachments)
        .unwrap_or_default()
        .into_inner())
}
