# bb - CLI-based bookmark manager for nerds


## Overview

**bb** is a bookmark manager for people who like to collect shit only to never revisit it again. It comes with a full-featured web UI and a CLI, supports image previews, and automatically scrapes page metadata. Run it as a standalone CLI tool or deploy as a daemon on a remote server.

![Main view](https://github.com/user-attachments/assets/bc86ff85-d9ca-4baa-b5f5-f139664a0421)

**This project is heavily work-in-progress!**

***bb** is inspired by [buku](https://github.com/jarun/buku).*

## Features

- **Tags**: Categorize bookmarks with tags. Tags are hierarchical — use `/` to create nested categories (e.g. `dev/rust`, `dev/python`). Searching for a parent tag matches all children: filtering by `dev` also matches `dev/rust` and `dev/python`. This applies to both tag filters and the `#` query prefix.
- **Rules**: Create custom rules using YAML configuration. Define matching queries for titles, URLs, or descriptions, and apply actions based on those matches. Rules also support an optional `query` condition — a search query string evaluated via the search query language (supports `#tag`, `.title`, `>desc`, `:url`, `and`, `or`, `not`, quoted phrases, parens). The query condition is AND-ed with other conditions. For example, bb can automatically assign tag "dev" for every url containing "github.com", or use `query: "#work and :github.com"` to match bookmarks tagged "work" with GitHub URLs. Rules can be managed via the Web UI Settings panel or the CLI.
- **Scrape Metadata**: When you create a bookmark, bb fetches metadata through a multi-stage pipeline: URLs are normalized (tracking params stripped, hosts lowercased), then bb fans out parallel requests to oEmbed, Plain HTML, Microlink, and Peekalink fetchers. Results are merged field-by-field by priority. Images are validated via magic byte detection, decode check, and minimum resolution (>32x32) to filter out tracking pixels and corrupt data. Headless Chrome is used as fallback when no validated image is found. The Chrome instance includes fingerprint spoofing (deviceMemory, maxTouchPoints, WebGL vendor/renderer, AudioContext) to bypass bot detection. Failed metadata tasks are retried up to 3 times (configurable) with exponential backoff (5s × 2^attempt + jitter) for transient errors (5xx, timeout); 4xx errors are terminal. You can also upload custom cover images and favicons per bookmark via the Web UI.
- **Web UI**: Manage your bookmarks through a user-friendly web interface built with Vite, React, and shadcn/ui. Stores screenshots and favicons for quick reference. Installable as a PWA with share target and protocol handler support — share URLs directly from your browser or OS into bb.
- **Workspaces**: Organize bookmarks into filtered views. Each workspace defines tag whitelist/blacklist and an optional filter query. Bookmarks matching the workspace filters appear automatically. Workspaces are persisted in `workspaces.yaml` and managed via the Web UI settings panel or the REST API. Drag-and-drop reordering is supported.
- **Bulk Operations**: Edit or delete multiple bookmarks at once. Bulk actions apply to all bookmarks matching the current search query — add, remove, or overwrite tags, and update fields in batch. Available from the toolbar in the Web UI.
- **Editor Mode**: Use `bb add --editor` to open your `$EDITOR` with a structured template for filling in URL, title, tags, and description. Leave a field as `-` to skip auto-fill for that field. Works with any editor — vim, nvim, nano, etc.
- **Standalone CLI Tool or Daemon**: Run bb as a standalone CLI tool or deploy it as a daemon on a remote server. Use the bb-cli as a lightweight client to connect to the server over HTTP.
- **Semantic Search** *(experimental)*: Find bookmarks by meaning rather than exact text matches. Currently inaccurate for most workloads — prefer text search for reliable results. Disabled by default.

![List view](https://github.com/user-attachments/assets/5d92eea4-d097-49c5-af8d-4bd25a7c6069)
![Bookmark detail](https://github.com/user-attachments/assets/7ab2c129-1922-4360-97c7-fedb77c2cb04)
![Settings](https://github.com/user-attachments/assets/b76abb51-62b2-49f8-946e-6e8dcec12b7d)
![Create bookmark](https://github.com/user-attachments/assets/b1b7325a-e9aa-4198-9eec-f3ca490ba963)
![CLI](https://github.com/bbonvi/bb/blob/main/screenshots/shot1.png?raw=true)

## Search Query Language

The `query` field supports a structured query language with field prefixes, boolean operators, quoted phrases, and grouping.

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

## Web UI Tips

- **Paste a URL anywhere** to instantly open the create bookmark modal with the URL pre-filled (as long as focus isn't in a text field).
- **Ctrl+N** opens the create bookmark modal.
- **Ctrl+Enter** saves edits in the detail modal.
- **Left/Right arrow keys** navigate between bookmarks in the detail view.
- **Click any tag** to add it as a search filter.
- **Double-click the delete button** to confirm deletion (first click arms, second executes).
- **Drag & drop or paste images** onto the cover or favicon area in edit mode to upload custom images.
- **URL parameters are persisted** — any query parameters in the URL are applied on load. Supported params: `workspace`, `query`, `tags`, `title`, `url`, `description`, `semantic`, `all`. For example, `/?workspace=Dev&query=rust&tags=lang` opens the Dev workspace with "rust" in the query field and "lang" as a tag filter. Bookmark this URL to always start with a specific view.
- **`?action=create`** — opens the create bookmark modal on page load. Combine with other params to pre-fill fields: `/?action=create&url=https://example.com&title=Example&tags=reading` opens the modal with those values already populated.

## Settings

The Web UI settings panel (gear icon) has two sections:

- **Preferences**
  - **Show catch-all workspace** — display a "---" option in the workspace selector to view all bookmarks regardless of workspace.
  - **Polling intervals** — configure how often the UI refreshes data in the background (separate intervals for normal, busy, and hidden-tab states).
  - **Globally ignored tags** — bookmarks with these tags are completely hidden everywhere in the UI. Useful for archiving or soft-deleting without removing data.

- **Workspaces** — create, edit, reorder, and delete workspaces. Each workspace defines tag whitelists/blacklists and an optional filter query.

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
   - **Lexical ranking** — text matching against title, description, and tags
4. Results are merged using Reciprocal Rank Fusion (RRF) — items appearing in both rankings get boosted
5. Other search criteria (tags, title filters) are applied before semantic ranking

### Configuration

```yaml
semantic_search:
  enabled: true                    # Enable/disable semantic search
  model: "all-MiniLM-L6-v2"       # Embedding model (default, ~23MB)
  default_threshold: 0.35          # Minimum similarity score (0.0-1.0)
  download_timeout_secs: 300       # Model download timeout
  semantic_weight: 0.6             # Balance between semantic and lexical ranking
```

**Available models**: `all-MiniLM-L6-v2` (default), `bge-small-en-v1.5`, `bge-base-en-v1.5`, `bge-large-en-v1.5` (and quantized variants).

### Notes

- First search downloads the model (~23MB for default)
- Models cached at `~/.local/share/bb/models/`
- Combine with other filters: `bb search --sem "tutorials" --tags dev`
- Web UI shows semantic input when feature is enabled

## Scrape Configuration

Controls URL fetching behavior for metadata scraping:

```yaml
scrape:
  # Accept invalid TLS certificates (default: false)
  accept_invalid_certs: false

  # Allowed URL schemes for fetching (default: ["http", "https"])
  allowed_schemes:
    - http
    - https

  # Blocked hostnames (default: [])
  blocked_hosts: []

  # Block requests to private/loopback IP ranges (default: true)
  # Prevents SSRF attacks by rejecting 127.0.0.1, 192.168.x.x, etc.
  block_private_ips: true

# Maximum retries for failed metadata fetches (default: 3, range: 1-10)
# Only transient errors (5xx, timeout, connection) are retried with exponential backoff
task_queue_max_retries: 3
```

Configuration options:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `scrape.accept_invalid_certs` | bool | `false` | Accept invalid TLS certificates when fetching URLs |
| `scrape.allowed_schemes` | list | `["http", "https"]` | Allowed URL schemes (e.g., http, https, ftp) |
| `scrape.blocked_hosts` | list | `[]` | Blocked hostnames — requests to these hosts will be rejected |
| `scrape.block_private_ips` | bool | `true` | Block requests to private/loopback IP ranges for SSRF protection |
| `task_queue_max_retries` | int | `3` | Max retries for transient metadata fetch failures (5xx, timeout); 4xx errors are terminal |

### Metadata Fetching Pipeline

When scraping metadata, bb executes the following stages:

1. **URL Normalization**: Tracking parameters stripped (utm_*, fbclid, gclid, etc.), hosts lowercased, trailing slashes removed, protocol-relative URLs resolved
2. **Parallel Fetching**: oEmbed, Plain HTML, Microlink, Peekalink, and Iframely fetchers run concurrently via thread pool
3. **oEmbed Support**: Checks URL against provider registry (cached from oembed.com/providers.json with hardcoded fallback for top 15 providers). Supports YouTube, Vimeo, Twitter, Spotify, SoundCloud, TikTok, etc.
4. **Field Merging**: Results merged by priority (oEmbed > HTML > Microlink > Peekalink > Iframely)
5. **Image Validation**: Fetched images validated via magic byte detection (PNG/JPEG/WebP/GIF), decode check, minimum resolution >32x32. Rejects tracking pixels, HTML responses, corrupt data
6. **Headless Chrome Fallback**: Launched only when no validated image is found. Includes stealth fingerprinting (deviceMemory, maxTouchPoints, WebGL vendor/renderer, AudioContext)
7. **Retry Logic**: Failed tasks retried with exponential backoff (5s × 2^attempt + jitter) up to `task_queue_max_retries` for transient errors (5xx, timeout, connection); 4xx errors are terminal

## Data Management

### Backup & Restore

Create portable backups of your bb data:

```bash
# Create backup archive (default: ./bb-backup-{timestamp}.tar.gz)
bb backup

# Create backup at specific path
bb backup /path/to/backup.tar.gz

# Pipe backup to stdout (auto-detected when stdout is not a terminal)
bb backup > backup.tar.gz
docker compose run --rm bb bb backup > backup.tar.gz

# Restore from backup (prompts for confirmation)
bb import /path/to/backup.tar.gz

# Restore without confirmation
bb import /path/to/backup.tar.gz --yes

# Pipe backup into import (auto-detected when stdin is piped)
cat backup.tar.gz | bb import
docker compose run --rm -i bb bb import < backup.tar.gz
```

**Included in backups:**
- `bookmarks.csv` — All bookmark data
- `config.yaml` — Configuration (user settings, read-only at runtime)
- `rules.yaml` — Automated rules (managed by the application)
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

### Docker Compose (preferred)

```bash
git clone https://github.com/bbonvi/bb.git
cd bb
cp .env.example .env
# Edit .env and set BB_AUTH_TOKEN (required)

# Production
docker compose up -d

# Development (cargo watch + vite dev server)
docker compose -f docker-compose.dev.yml up
```

See [Running daemon in docker](#running-daemon-in-docker) for details.

### Manual build

Requires [Rust](https://www.rust-lang.org/) and optionally [Node.js](https://nodejs.org/) for the web UI.

```bash
git clone https://github.com/bbonvi/bb.git
cd bb

# Build backend
cargo build --release
sudo mv ./target/release/bb /usr/local/bin/bb

# Build web-ui (optional)
cd client
yarn install
yarn build
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

**Production** (`docker-compose.yml`):
```bash
docker compose up -d
docker compose logs -f   # view logs
docker compose down      # stop
```

Includes automatic restarts, health checks, memory limits (2GB), log rotation, and Cloudflare DNS.

**Development** (`docker-compose.dev.yml`):
```bash
docker compose -f docker-compose.dev.yml up
# Backend: http://localhost:8080
# Frontend: http://localhost:3000
```

Mounts source directories — backend recompiles via `cargo watch`, frontend hot-reloads via Vite.

**Standalone docker (without compose):**
```bash
docker build -t bb:latest -f daemon.Dockerfile .
docker run --rm -it -v bb-data:/root/.local/share/bb -p 8080:8080 --name bb-daemon bb:latest
```

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
| `IFRAMELY_API_KEY`    | Iframely API key for rich metadata extraction         |                       | `abc123...`             |


