use futures_util::StreamExt;
use indicatif::ProgressStyle;
use reqwest::header;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

use crate::parse::Attachment;
use crate::{CLIENT, PB, SEM};
pub async fn download_attachments(
    url: &str,
    output: &str,
    attachments: Vec<Attachment>,
) -> anyhow::Result<()> {
    let url = reqwest::Url::parse(url)
        .map_err(|e| anyhow::anyhow!("无效的 URL: {}，错误: {}", url, e))?;
    let domain = Arc::new(
        url.host_str()
            .ok_or_else(|| anyhow::anyhow!("无法解析域名: {}", url))?
            .to_string(),
    );
    let username = Arc::new(
        url.path_segments()
            .and_then(|mut segments| segments.next_back())
            .ok_or_else(|| anyhow::anyhow!("无法获取用户名"))?
            .to_string(),
    );

    let video_pbar = Arc::new(PB.add(indicatif::ProgressBar::new(attachments.len() as u64)));
    video_pbar.set_style(
        ProgressStyle::default_bar()
            .template("[{wide_bar:.green/white}] [{pos}/{len}]")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut tasks = JoinSet::new();
    for att in attachments {
        let video_pbar = Arc::clone(&video_pbar);
        let output = output.to_string();
        let username = Arc::clone(&username);
        let domain = Arc::clone(&domain);

        tasks.spawn(async move {
            let _permit = SEM.acquire().await;

            let folder = format!("{}/{}", output, username);
            let _ = fs::create_dir_all(&folder).await;
            let path = format!("{}/{}", folder, att.name);
            let mut downloaded = 0u64;
            if Path::new(&path).exists() {
                downloaded = fs::metadata(&path).await.unwrap().len();
            }
            let mut req = CLIENT.get(format!(
                "{}/data{}",
                att.server.as_ref().unwrap_or(&format!(
                    "https://{}",
                    domain
                )),
                att.path
            ));
            if downloaded > 0 {
                req = req.header(header::RANGE, format!("bytes={}-", downloaded));
            }


            let resp = match req.send().await {
                Ok(resp) => resp,
                Err(err) => {
                    let _ = PB.println(format!("下载失败: {} - {}", att.name, err));
                    return;
                }
            };

            let pb = PB.add(indicatif::ProgressBar::new(
                downloaded + resp.content_length().unwrap_or(0),
            ));
            pb.set_message(format!("正在下载: {}", att.name));
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
            pb.set_style(
                ProgressStyle::with_template(
                    "[{elapsed_precise}] [{wide_bar:.magenta/blue}] [{decimal_bytes}/{decimal_total_bytes}] [{decimal_bytes_per_sec}] {eta}🔄"
                )
                .unwrap()
                .progress_chars("#>-"),
            );

            if resp.status() == 416 {
                let _ = PB.println(format!(
                    "文件已下载完成: {} - {}",
                    resp.status(),
                    resp.url()
                ));
                video_pbar.inc(1);
                pb.finish_and_clear();
                return;
            }
            if !(resp.status().is_success() || resp.status() == 206) {
                let _ = PB.println(format!(
                    "下载失败: {} - {}",
                    resp.status(),
                    resp.url()
                ));
                video_pbar.inc(1);
                pb.finish_and_clear();
                return;
            }
            if resp.status() == 206 {
                resp.headers()
                    .get(header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.split('/').next_back())
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(downloaded + resp.content_length().unwrap_or(0))
            } else {
                resp.content_length().unwrap_or(0)
            };

            let mut file = OpenOptions::new()
                .write(true)
                .create(true).truncate(true)
                .open(&path)
                .await.unwrap();
            let mut pos = downloaded;
            pb.set_length(pos + resp.content_length().unwrap_or(0));
            let mut resp_stream = resp.bytes_stream();
            while let Some(chunk) = resp_stream.next().await {
                match chunk {
                    Ok(chunk) => {
                        let len = chunk.len();
                        pos += len as u64;
                        file.write_all(&chunk).await.unwrap();
                        pb.set_position(pos);
                    }
                    Err(err) => {
                        let _ = PB.println(format!("下载失败: {} - {}", att.name, err));
                        return;
                    }
                }
            }

            file.flush().await.unwrap();

            video_pbar.inc(1);
            pb.finish_and_clear();
        });
    }

    tasks.join_all().await;

    Ok(())
}
