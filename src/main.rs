use crate::{download::download_attachments, parse::parse_artist_url};
mod download;
mod parse;
use clap::Parser;
use indicatif::MultiProgress;
use reqwest::Client;
use std::sync::{Arc, LazyLock, OnceLock};
use tokio::sync::Semaphore;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

static CLIENT: OnceLock<Client> = OnceLock::new();
static PB: LazyLock<MultiProgress> = LazyLock::new(MultiProgress::new);
static SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Download videos from Kemono / Coomer and save to a folder"
)]
struct Args {
    /// Kemono / Coomer Artist 的 URL
    url: String,

    /// Folder to save downloaded videos
    #[arg(short, long, default_value = "./download")]
    output: String,

    /// Proxy URL (optional)
    /// Example: socks5://192.168.2.1:7890
    #[arg(short, long)]
    proxy: Option<String>,

    /// Number of retries for failed downloads
    /// Default is 3 retries
    #[arg(short, long, default_value = "3")]
    retries: u32,

    /// Number of concurrent downloads
    /// Default is 5 concurrent downloads
    #[arg(short, long, default_value = "5")]
    concurrent: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if !(args.url.contains("kemono") || args.url.contains("coomer")) {
        anyhow::bail!("URL must be a Kemono or Coomer artist page.");
    }

    // Set the number of concurrent downloads
    SEM.set(Arc::new(Semaphore::new(args.concurrent as usize)))
        .expect("Failed to set global semaphore");

    let retry_strategy = reqwest::retry::for_host(String::from("coomer.cr"))
        .max_retries_per_request(args.retries)
        .classify_fn(|req_rep| {
            if let Some(status) = req_rep.status() {
                if status.is_success() {
                    req_rep.success()
                } else {
                    req_rep.retryable()
                }
            } else {
                req_rep.retryable()
            }
        });

    let client = if let Some(proxy) = args.proxy {
        Client::builder()
            .user_agent(USER_AGENT)
            .proxy(reqwest::Proxy::all(proxy).expect("Failed to create proxy"))
            .retry(retry_strategy)
            .build()
            .expect("Failed to create HTTP client")
    } else {
        Client::builder()
            .user_agent(USER_AGENT)
            .retry(retry_strategy)
            .build()
            .expect("Failed to create HTTP client")
    };

    // Initialize the global HTTP client
    CLIENT
        .set(client)
        .expect("Failed to set global HTTP client");

    // Get all un-downloaded resource IDs
    let files = parse_artist_url(&args.url).await?;

    // Start downloading
    download_attachments(&args.url, &args.output, files).await?;

    Ok(())
}
