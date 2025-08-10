# ckdownloader

A command-line tool written in Rust to download attachments (videos, images, etc.) from Kemono and Coomer artist pages and save them to a local folder.

## Features

- Parse Kemono/Coomer artist pages
- Fetch details for each post
- Download attachments concurrently
- Support for HTTP proxies
- Progress bars for download status

## Prerequisites

- Rust and Cargo (Tested with Rust 1.70+)

## Installation

```bash
# Clone the repository
git clone https://github.com/TnZzZHlp/ckdownloader.git
cd ckdownloader

# Build the release binary
cargo build --release

```

## Usage

```bash
ckdownloader <ARTIST_URL> [OPTIONS]
```

### Arguments

- `<ARTIST_URL>`: URL of the Kemono or Coomer artist page (required).

### Options

- `-o, --output <OUTPUT>`: Folder to save downloaded attachments (default: `./download`).
- `-p, --proxy <PROXY>`: Proxy URL (e.g., `socks5://127.0.0.1:1080`).
- `-r, --retries <RETRIES>`: Number of retries for failed downloads (default: 3).
- `-c, --concurrency <CONCURRENCY>`: Number of concurrent downloads (default: 5).

## Examples

Download all attachments from an artist page:

```bash
ckdownloader --url https://kemono.su/patreon/user/150779108
```

Specify a custom output directory:

```bash
ckdownloader --url https://kemono.su/patreon/user/150779108 --output my_downloads
```

Use a SOCKS5 proxy:

```bash
ckdownloader --url https://kemono.su/patreon/user/150779108 --proxy socks5://127.0.0.1:7890
```

## Acknowledgments

This project makes use of:

- [clap](https://crates.io/crates/clap) for command-line argument parsing
- [reqwest](https://crates.io/crates/reqwest) for HTTP client
- [tokio](https://crates.io/crates/tokio) for asynchronous runtime
- [indicatif](https://crates.io/crates/indicatif) for progress bars
- [anyhow](https://crates.io/crates/anyhow) for error handling

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
