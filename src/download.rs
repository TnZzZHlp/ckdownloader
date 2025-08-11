use futures_util::StreamExt;
use indicatif::ProgressStyle;
use reqwest::header;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;

use crate::parse::File;
use crate::{CLIENT, PB, SEM};
pub async fn download_files(url: &str, output: &str, files: Vec<File>) -> anyhow::Result<()> {
    let url = reqwest::Url::parse(url)
        .map_err(|e| anyhow::anyhow!("Invalid URL: {}, Error: {}", url, e))?;
    let domain = Arc::new(
        url.host_str()
            .ok_or_else(|| anyhow::anyhow!("Unable to resolve domain name: {}", url))?
            .to_string(),
    );
    let username = Arc::new(
        url.path_segments()
            .and_then(|mut segments| segments.next_back())
            .ok_or_else(|| anyhow::anyhow!("Unable to retrieve username"))?
            .to_string(),
    );

    let video_pbar = Arc::new(PB.add(indicatif::ProgressBar::new(files.len() as u64)));
    video_pbar.set_style(
        ProgressStyle::default_bar()
            .template("[{wide_bar:.green/white}] [{pos}/{len}]")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut tasks = JoinSet::new();
    for file in files {
        let video_pbar = Arc::clone(&video_pbar);
        let output = output.to_string();
        let username = Arc::clone(&username);
        let domain = Arc::clone(&domain);

        tasks.spawn(async move {
            let file = file.clone();
            let _ = download(file, &output, &username, &domain).await;
            video_pbar.inc(1);
        });
    }

    tasks.join_all().await;

    Ok(())
}

async fn download(file: File, output: &str, username: &str, domain: &str) {
    let _permit = SEM.get().unwrap().acquire().await;

    let folder = format!("{}/{}", output, username);
    let _ = fs::create_dir_all(&folder).await;
    let path = format!("{}/{}/{}", folder, file.post_id.unwrap(), file.name);
    let mut downloaded = 0u64;
    if Path::new(&path).exists() {
        downloaded = fs::metadata(&path).await.unwrap().len();
    }

    let url = format!(
        "{}/data{}",
        file.server
            .as_ref()
            .unwrap_or(&format!("https://{}", domain)),
        file.path
    );

    let mut req = CLIENT.get().unwrap().get(&url);
    if downloaded > 0 {
        req = req.header(header::RANGE, format!("bytes={}-", downloaded));
    }

    let resp = match req.send().await {
        Ok(resp) => resp,
        Err(err) => {
            let _ = PB.println(format!("Download failed: {} - {}", url, err));
            return;
        }
    };

    let pb = PB.add(indicatif::ProgressBar::new(
        downloaded + resp.content_length().unwrap_or(0),
    ));
    pb.set_message(format!("Downloading: {}", file.name));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb.set_style(
                ProgressStyle::with_template(
                    "[{elapsed_precise}] [{wide_bar:.magenta/blue}] [{decimal_bytes}/{decimal_total_bytes}] [{decimal_bytes_per_sec}] {eta}🔄"
                )
                .unwrap()
                .progress_chars("#>-"),
            );

    if resp.status() == 416 {
        let _ = PB.println(format!("File download completed: {}", resp.url()));
        pb.finish_and_clear();
        return;
    }

    if !resp.status().is_success() {
        let _ = PB.println(format!(
            "Download failed: {} - {}",
            resp.status(),
            resp.url()
        ));
        pb.finish_and_clear();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("./error.txt")
            .await
            .unwrap();

        let mut contents = String::new();
        let _ = file.read_to_string(&mut contents).await;
        if !contents.contains(&url) {
            let _ = file
                .write_all(format!("{} - {}\n", url, resp.status()).as_bytes())
                .await;
        }

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

    if !Path::new(&path).parent().unwrap().exists() {
        let _ = fs::create_dir_all(Path::new(&path).parent().unwrap()).await;
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .await
        .unwrap();
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
                let _ = PB.println(format!("Download failed: {} - {}", url, err));
                return;
            }
        }
    }

    file.flush().await.unwrap();

    pb.finish_and_clear();
}
