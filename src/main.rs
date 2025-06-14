use crate::{
    download::download_attachments,
    parse::{get_details, parse_artist_url},
};
mod download;
mod parse;
use clap::Parser;
use indicatif::MultiProgress;
use reqwest::Client;
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .expect("Failed to create HTTP client")
});
static PB: LazyLock<MultiProgress> = LazyLock::new(MultiProgress::new);
static SEM: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

#[derive(Parser)]
#[command(author, version, about = "下载 Kemono / Coomer 的视频并保存到文件夹")]
struct Args {
    /// Kemono / Coomer Artist 的 URL
    url: String,
    /// 保存下载视频的文件夹
    #[arg(short, long, default_value = "./download")]
    output: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if !(args.url.contains("kemono") || args.url.contains("coomer")) {
        anyhow::bail!("URL 必须是 Kemono 或 Coomer 的 Artist 页面。");
    }

    // 获取所有未下载的资源 ID
    let all_ids = parse_artist_url(&args.url).await?;

    // 获取资源详情
    let atts = get_details(&args.url, all_ids).await?;

    // 开始下载
    download_attachments(&args.url, &args.output, atts).await?;

    Ok(())
}
