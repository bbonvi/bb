# bb - CLI-based bookmark manager for nerds


## Overview

**bb** is a CLI based bookmark manager designed for people who like to collect shit only to never revisit it again. It supports image previews and comes with a  simple webui. **bb** can be ran as a standalone CLI utility or deployed as a daemon on a remote server. Additionally, **bb** scrapes the web pages for you in order to retrieve metadata. 

![](https://github.com/bbonvi/bb/blob/main/screenshots/shot1.png?raw=true)
![](https://github.com/bbonvi/bb/blob/main/screenshots/shot2.png?raw=true)

**This project is heavily work-in-progress!**

***bb** is inspired by [buku](https://github.com/jarun/buku).*


## Features

- **Tags**: Categorize bookmarks with tags. Tags are hierarchical — use `/` to create nested categories (e.g. `dev/rust`, `dev/python`). Searching for a parent tag matches all children: filtering by `dev` also matches `dev/rust` and `dev/python`. This applies to both tag filters and the `#` keyword prefix.

- **Rules**: Create custom rules using YAML configuration. Define matching queries for titles, URLs, or descriptions, and apply actions based on those matches. For example, bb can automatically assign tag "dev" for every url containing "github.com".

- **Scrape Metadata**: When you create a bookmark, bb attempts to fetch metadata from the page via a simple GET request. It extracts the title, description, and URL for page thumbnails (og:image metadata). If the request fails, bb will launch a headless chromium instance to retrieve the same information and take a screenshot of the page as well as favicon. Additionally, the chrome instance will attempt to bypass captchas.

- **Web UI**: Manage your bookmarks through a user-friendly web interface. This feature is particularly useful as bb stores screenshots and favicons of your pages for quick reference.

- **Standalone CLI Tool or Daemon**: Run bb as a standalone CLI tool or deploy it as a daemon on a remote server. Use the bb-cli as a lightweight client to connect to the server over HTTP.

- **Semantic Search** *(experimental)*: Find bookmarks by meaning rather than exact keywords. Currently inaccurate for most workloads — prefer keyword search for reliable results. Disabled by default.

## Keyword Search Query Language

The `keyword` field supports a structured query language with field prefixes, boolean operators, quoted phrases, and grouping.

### Field Prefixes

| Prefix | Field | Example |
|--------|-------|---------|
| `#` | tags | `#video` — exact tag match + hierarchical (`#programming` matches `programming/rust`) |
| `.` | title | `.youtube` — substring, case-insensitive |
| `>` | description | `>tutorial` — substring, case-insensitive |
| `:` | url | `:github.com` — substring, case-insensitive |
| (none) | all fields | `video` — substring across title, description, url, tags |

### Quoted Phrases

Group multiple words into a single term: `."getting started"`, `>"deploy with docker"`, `"async runtime"`.

### Boolean Operators

- `and` — both must match (implicit when terms are space-separated)
- `or` — either must match
- `not` — negates the following term/group
- Precedence: `not` > `and` > `or`
- To search reserved words literally, quote them: `"and"`, `"or"`, `"not"`

### Parentheses

Group sub-expressions: `(#video and .youtube) or (#audio and .spotify)`

### Backslash Escaping

Search prefix characters literally: `\#hashtag`, `\:colon`, `\.dot`, `\>arrow`

### Examples

```
rust async                        → bookmarks mentioning both "rust" and "async" anywhere
#recipe not #dessert              → tagged "recipe" but not "dessert"
."getting started"                → title contains the phrase "getting started"
>"deploy to production"           → description contains "deploy to production"
:stackoverflow.com                → URL contains "stackoverflow.com"
#dev/backend or #dev/frontend     → hierarchical tag match on either subtree
(#python or #rust) .tutorial      → tutorials tagged python or rust
not #read :arxiv.org              → unread papers from arxiv
```

## Semantic Search

bb includes local semantic search powered by [fastembed](https://github.com/Anush008/fastembed-rs) and ONNX models. Embeddings are generated locally—no external API calls.

### Quick Start

1. Enable in config (`~/.local/share/bb/config.yaml`):
   ```yaml
   semantic_search:
     enabled: true
   ```

2. Search by meaning:
   ```bash
   # CLI
   bb search --sem "machine learning tutorials"

   # With similarity threshold (0.0-1.0, default 0.35)
   bb search --sem "web development" --threshold 0.5
   ```

### How It Works

1. When you create or update a bookmark, bb generates an embedding from the title, description, tags, and URL domain
2. Embeddings are stored in `vectors.bin` alongside your bookmarks
3. Hybrid search combines two ranking methods:
   - **Semantic ranking** — cosine similarity between query and bookmark embeddings
   - **Lexical ranking** — keyword matching against title, description, and tags
4. Results are merged using Reciprocal Rank Fusion (RRF) — items appearing in both rankings get boosted
5. Other search criteria (tags, title filters) are applied before semantic ranking

### Configuration

```yaml
semantic_search:
  enabled: true                    # Enable/disable semantic search
  model: "all-MiniLM-L6-v2"       # Embedding model (default, ~23MB)
  default_threshold: 0.35          # Minimum similarity score (0.0-1.0)
  embedding_parallelism: 4         # Concurrent embeddings (0 = auto)
  download_timeout_secs: 300       # Model download timeout
```

**Available models**: `all-MiniLM-L6-v2` (default), `bge-small-en-v1.5`, `bge-base-en-v1.5`, `bge-large-en-v1.5` (and quantized variants).

### Notes

- First search downloads the model (~23MB for default)
- Models cached at `~/.local/share/bb/models/`
- Combine with other filters: `bb search --sem "tutorials" --tags dev`
- Web UI shows semantic input when feature is enabled

## Data Management

### Backup & Restore

Create portable backups of your bb data:

```bash
# Create backup archive (default: ./bb-backup-{timestamp}.tar.gz)
bb backup

# Create backup at specific path
bb backup /path/to/backup.tar.gz

# Restore from backup (prompts for confirmation)
bb import /path/to/backup.tar.gz

# Restore without confirmation
bb import /path/to/backup.tar.gz --yes
```

**Included in backups:**
- `bookmarks.csv` — All bookmark data
- `config.yaml` — Configuration
- `workspaces.yaml` — Workspace definitions
- `uploads/` — Preview images and favicons

### Image Compression

Convert existing preview images to WebP format for reduced storage:

```bash
# Preview what would be compressed
bb compress --dry-run

# Compress images (prompts for confirmation)
bb compress

# Compress without confirmation
bb compress --yes
```

## Installation

*There are no precompiled binaries for now.*

To install bb, follow these steps:

1. Ensure you have Rust installed on your machine. If not, you can install it from [rust-lang.org](https://www.rust-lang.org/).

2. Clone the repository:

   ```bash
   git clone https://github.com/bbonvi/bb.git
   cd bb
   ```

3. Build the project:

   ```bash
   cargo build --release
   # move binary to your PATH
   sudo mv ./target/release/bb /usr/local/bin/bb
   ```

4. Build web-ui (optional):

   ```bash
   cd client
   yarn
   yarn run build --release
   cd ../
   ```
   
## Usage

### Standalone CLI:
   
   ```bash
    # this will create bookmark and attempt to fetch metadata
    bb add --url "https://github.com/bbonvi/bb"

    # This will open up a neovim window where you can fill up the details
    EDITOR=nvim bb add --editor

    # output all bookmarks
    bb search
   ```

### Daemon:

   ```bash
    # start the daemon
    RUST_LOG=info bb daemon

    # this will connect to bb daemon at localhost:8080 and create a bookmark.
    # --async-meta parameter makes it so daemon fetches metadata in background
    # and you immediately get a response back, without a wait.
    BB_ADDR=http://localhost:8080 bb add --async-meta --url "https://github.com/bbonvi/bb"

    # after daemon completes the fetch, you can query bookmark by its title
    BB_ADDR=http://localhost:8080 bb search --title bb
   ```

### Running daemon in docker

**Quick start:**
```bash
# build with headless chrome
docker build -t bb:latest -f daemon.Dockerfile .

# run and open http://localhost:8080
docker run --rm -it -v bb-data:/root/.local/share/bb -p 8080:8080 --name bb-daemon bb:latest
```

**Production deployment with docker-compose:**
```bash
# Copy and configure environment
cp .env.example .env
# Edit .env and set BB_AUTH_TOKEN (required)

# Build and start
docker compose up -d

# View logs
docker compose logs -f

# Stop
docker compose down
```

The compose file includes:
- Automatic restarts
- Health checks
- Memory limits (2GB)
- Log rotation
- Cloudflare DNS (1.1.1.1)

### WebUI

When running bb as daemon, you can access webui at [http://localhost:8080/](http://localhost:8080/)

### Authentication

bb supports optional bearer token authentication for API routes. When enabled, both the web UI and CLI client require a valid token to access the API.

**Enable authentication:**

```bash
# Set a secure token (16+ characters recommended)
BB_AUTH_TOKEN=your-secret-token-here bb daemon
```

**CLI client with auth:**

```bash
BB_ADDR=http://localhost:8080 BB_AUTH_TOKEN=your-secret-token-here bb search
```

**Behavior:**
- When `BB_AUTH_TOKEN` is unset or empty, authentication is disabled (backwards compatible)
- When set, all `/api/*` requests require `Authorization: Bearer <token>` header
- Static files (web UI assets) are always accessible without authentication
- The web UI will prompt for the token on first access when auth is enabled 


# API

### CLI API references
[API.md](https://github.com/bbonvi/bb/blob/main/API.md)

### Environment Variables

| Variable      | Description      | Default      | Example |
| ------------- | ---------------- | ------------ | ------- |
| `RUST_LOG`            | log level | error | warn |
| `BB_BASE_PATH`        | Base path for bookmarks, configs and thumbnails       | `~/.local/share/bb`   | `~/.local/share/bb`     |
| `BB_ADDR`             | Daemon http address                                   |                       | `http://localhost:8080` |
| `BB_AUTH_TOKEN`       | Bearer token for API authentication. When set, all API requests require `Authorization: Bearer <token>` header. |                       | `my-secret-token-1234` |
| `BB_BASIC_AUTH`       | Optional basic auth for daemon authorization (deprecated, use `BB_AUTH_TOKEN`). |                       | `myusername:mypassword` |
| `HTTP_PROXY`          | Proxy for all meta requests                           |                       | `socks5://127.0.0.1:8060` |
| `OPT_PROXY`           | An optional proxy that will be used in case default (no proxy/HTTP_PROXY) request fails. Useful if bb needs to access region locked website, but you don't want to increase the probability of captcha. | | `socks5://127.0.0.1:8060` |
| `CHROME_PATH`         | A path to chromium binary                             | `chromium`            | `/usr/sbin/chromium`    |
| `EDITOR`              | Your default text editor                              | `vim`                 | `nvim`                  |
| `SHELL`               | Shell to launch editor with                           | `/usr/sbin/bash`      | `/bin/bash`             |


