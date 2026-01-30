# Architecture Overview

This document describes the internal architecture of bb, intended for developers navigating the codebase.

## High-Level Structure

```
┌─────────────────────────────────────────────────────────┐
│                    Entry Points                         │
│   CLI (main.rs)              Daemon (web.rs)            │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                   CLI Layer (cli/)                      │
│   commands.rs, handlers.rs, validation.rs               │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              AppService (app/service.rs)                │
│   Business logic, semantic search orchestration         │
└────────────────────────┬────────────────────────────────┘
                         │
        ┌────────────────┴───────────────┐
        │                                │
┌───────▼─────────────────┐    ┌─────────▼───────────────┐
│   AppBackend trait      │    │ SemanticSearchService   │
│   AppLocal / AppRemote  │    │ (optional, local only)  │
└───────┬─────────────────┘    └─────────────────────────┘
        │
   ┌────┴─────────────┐
   │                  │
┌──▼────────────┐  ┌──▼──────────────┐
│BookmarkManager│  │StorageManager   │
│(BackendCsv)   │  │(BackendLocal)   │
└───────────────┘  └─────────────────┘
```

## Execution Modes

bb supports two deployment modes:

| Mode | Trigger | Description |
|------|---------|-------------|
| **Local** | Default | CLI operates directly on local CSV/files |
| **Remote** | `BB_ADDR` set | CLI proxies requests to daemon via HTTP |

In remote mode, the daemon handles all storage and semantic search; the CLI is a thin HTTP client.

---

## Core Components

### 1. Application Layer (`src/app/`)

**factory.rs** — Composition root. Creates `AppService` based on environment:
- Local mode: wires up storage, config, bookmark manager, optional semantic service
- Remote mode: creates HTTP client backend

**service.rs** — `AppService` orchestrates business logic:
- Validates inputs (duplicate URLs, field constraints)
- Coordinates backend operations with semantic indexing
- Applies search filters then semantic ranking

**backend.rs** — `AppBackend` trait defines the core interface:
```rust
pub trait AppBackend: Send + Sync {
    fn create(&self, bookmark, opts) -> Result<Bookmark>;
    fn search(&self, query) -> Result<Vec<Bookmark>>;
    fn update(&self, id, update) -> Result<Bookmark>;
    fn delete(&self, id) -> Result<()>;
    // ...
}
```

**local.rs** — `AppLocal` implements full local operations with task queue for async metadata fetching.

**remote.rs** — `AppRemote` proxies all calls to daemon HTTP API.

### 2. Storage Layer

**bookmarks.rs** — `BackendCsv` manages bookmark persistence:
- CSV file at `~/.local/share/bb/bookmarks.csv`
- In-memory `Vec<Bookmark>` with `Arc<RwLock<>>` for concurrency
- Atomic writes via temp file + rename

**storage.rs** — `BackendLocal` manages binary assets:
- Images and icons stored in `~/.local/share/bb/uploads/`
- Atomic writes for crash safety

### 3. Search Query Language (`src/search_query/`)

Structured query parser for the `query` search field. Replaces simple whitespace-split matching with a full query language.

```
src/search_query/
├── mod.rs      # Public API: parse() → SearchFilter, eval()
├── lexer.rs    # Tokenizer: input string → Token stream
├── parser.rs   # Recursive descent: Token stream → AST (SearchFilter)
├── eval.rs     # Evaluates SearchFilter against a Bookmark
└── tests.rs    # Unit tests
```

- **Field prefixes**: `#tag`, `.title`, `>description`, `:url`, bare = all fields
- **Boolean operators**: `and`, `or`, `not` with standard precedence (`not` > `and` > `or`)
- **Implicit AND**: space-separated terms are AND-joined
- **Quoted phrases**: `."multi word term"`
- **Parenthesized grouping**: `(#a or #b) and .title`
- **Backslash escaping**: `\#literal` searches prefix characters literally

Tag matching is exact + hierarchical (`#dev` matches tag `dev/rust`). All other fields use case-insensitive substring matching.

Called from `BackendCsv::search()` when a `query` field is present on the search request.

### 4. Configuration (`src/config.rs`)

Configuration is split into two files:

- **`config.yaml`** — User settings (effectively read-only at runtime). Contains `task_queue_max_threads`, `task_queue_max_retries`, `semantic_search`, `images`, `scrape`.
- **`rules.yaml`** — Automated rules (machine-managed via `RulesConfig`). Separated to preserve user comments in `config.yaml`, since rules are the only frequently mutated data.

On first load after upgrade, rules are automatically migrated from `config.yaml` to `rules.yaml`.

```yaml
# config.yaml
task_queue_max_threads: 4
task_queue_max_retries: 3
semantic_search:
  enabled: false
  model: "all-MiniLM-L6-v2"
  default_threshold: 0.35
  semantic_weight: 0.6
scrape:
  accept_invalid_certs: false
  allowed_schemes:
    - http
    - https
  blocked_hosts: []
  block_private_ips: true
```

```yaml
# rules.yaml
rules:
- url: example.com
  action: !UpdateBookmark
    tags:
    - example
```

Config validation returns `Result<(), Vec<String>>` for proper error propagation. Invalid configs fail early with descriptive messages listing all validation errors.

**URL Policy Enforcement** (`ScrapeConfig`):
- Scheme validation: only whitelisted schemes (default: `http`, `https`) are allowed
- Host blocking: explicitly blocked hostnames are rejected
- SSRF protection: private/loopback IP ranges blocked by default (127.0.0.1, 192.168.x.x, 10.x.x.x, 172.16.x.x, fc00::/7, etc.)

**Task Queue Retry Settings**:
- `task_queue_max_retries`: max retry attempts for failed metadata fetches (default: 3, range: 1-10)
- Only transient errors (5xx, timeout, connection) are retried with exponential backoff
- 4xx errors and validation failures are terminal (no retry)

### 5. Workspace Storage (`src/workspaces.rs`)

YAML-persisted workspace definitions at `~/.local/share/bb/workspaces.yaml`:
- `WorkspaceStore` holds `Vec<Workspace>` in `Arc<RwLock<>>` (same concurrency pattern as config)
- Atomic writes via `BackendLocal::write`
- Auto-creates empty file on first load
- Validation: name non-empty/trimmed/max 100 chars, query must parse, no duplicate names (case-insensitive)
- ID generation via `Eid` (ULID-based)
- Workspace filtering uses server-side query search; the client translates tag whitelist/blacklist + query into a search query string

### 6. CLI Layer (`src/cli/`)

**handlers.rs** — Entry points called from `main.rs`:
- `handle_search()`, `handle_add()`, `handle_rule()`, etc.

**commands.rs** — Encapsulated command execution:
- `SearchCommand`, `AddCommand`, `MetaCommand`, `RuleCommand`
- Each validates inputs, executes via AppService, formats output

**validation.rs** — Input validation:
- Search query requires at least one criterion
- Semantic threshold in [0.0, 1.0]
- Tag format validation (no spaces, max length)

### 7. Web/API Layer (`src/web.rs`)

Daemon HTTP server (port 8080):

| Route | Method | Auth | Purpose |
|-------|--------|------|---------|
| `/api/bookmarks/search` | POST | Yes | Search with optional semantic ranking |
| `/api/bookmarks/create` | POST | Yes | Create bookmark |
| `/api/bookmarks/update` | POST | Yes | Update bookmark |
| `/api/bookmarks/delete` | POST | Yes | Delete bookmark |
| `/api/semantic/status` | GET | Yes | Semantic search feature status |
| `/api/config` | GET/POST | Yes | Read/update config |
| `/api/file/{id}` | GET | Yes | Serve uploaded images |
| `/api/workspaces` | GET | Yes | List all workspaces |
| `/api/workspaces` | POST | Yes | Create workspace |
| `/api/workspaces/:id` | PUT | Yes | Update workspace |
| `/api/workspaces/:id` | DELETE | Yes | Delete workspace |
| `/api/health` | GET | No | Health check |

Authentication via `BB_AUTH_TOKEN` env var; constant-time token comparison.

### 8. Task Queue (`src/app/task_runner.rs`)

Background metadata fetching for `--async-meta` flag:
- Tasks persisted to `task-queue.json` for recovery
- Worker thread pool with configurable concurrency
- Graceful shutdown on SIGTERM
- Retry logic with exponential backoff (5s × 2^attempt + jitter) up to `task_queue_max_retries` (default 3)
- Smart retry: only transient errors (5xx, timeout, connection) are retried; 4xx errors are terminal
- Task state tracking: pending → running → completed/failed
- Errors classified as Retryable or Terminal for smarter scheduling

### 9. Metadata Scraping (`src/metadata/`)

Multi-stage metadata fetching pipeline with parallel sources and validation:

```
src/metadata/
├── mod.rs           # Public API: fetch_metadata()
├── normalize.rs     # URL normalization (tracking params, case, trailing slash)
├── oembed.rs        # oEmbed provider registry and fetcher
├── fetchers/        # Parallel metadata sources (order configurable via scrape.fetcher_order)
│   ├── html.rs      # Plain HTML parser (og:*, meta tags, JSON-LD)
│   ├── wayback.rs   # Wayback Machine (archive.org) snapshot fetcher
│   ├── ddg.rs       # DuckDuckGo API client
│   ├── microlink.rs # Microlink API client
│   ├── peekalink.rs # Peekalink API client
│   └── iframely.rs  # Iframely API client
├── merge.rs         # Field-by-field priority merging with smart defaults
├── validate.rs      # Image validation (magic bytes, decode, resolution)
├── chrome.rs        # Headless Chrome fallback with stealth
└── error.rs         # FetchError types (Retryable/Terminal)
```

**Data Flow**:

```
URL
 → normalize_url()
     - Strip tracking params (utm_*, fbclid, gclid, etc.)
     - Lowercase host, trim trailing slash
     - Resolve protocol-relative URLs (//example.com → https://example.com)
 → thread::scope() parallel fetch:
     ├─→ oEmbed fetcher (checks provider registry)
     ├─→ Wayback Machine (archive.org snapshot)
     ├─→ Plain HTML fetcher (og:title, twitter:*, meta tags, JSON-LD structured data, <link rel="canonical">)
     ├─→ Microlink API
     ├─→ Peekalink API
     ├─→ Iframely API
     └─→ DDG API fetcher
 → merge_metadata(results)
     - Priority from config: scrape.fetcher_order (default: oEmbed > Wayback > Plain > Microlink > Peekalink > DDG)
     - Smart title fallback: og:title > twitter:title > JSON-LD > <title> tag
     - Smart description: generic/empty descriptions overridden by real content from lower-priority fetchers
     - Title validity: rejects site-wide defaults, error pages, bare domains, short generic strings
 → validate_image(image_url)
     - Magic byte detection (PNG/JPEG/WebP/GIF)
     - Image decode check (image-rs)
     - Minimum resolution >32x32
     - Reject tracking pixels, HTML responses, corrupt data
 → if no valid image:
     - Launch headless Chrome
     - Stealth fingerprinting (deviceMemory, maxTouchPoints, WebGL, AudioContext)
     - Screenshot + favicon extraction
```

**HTML Parsing** (`get_data_from_page()`):
- Extracts `og:title`, `twitter:title`, `twitter:description` meta tags
- Parses `<link rel="canonical">` for canonical URL
- Extracts JSON-LD structured data from `<script type="application/ld+json">`
- JSON-LD helper handles @graph arrays, top-level arrays, and extracts: name, headline, description, image (string/object/array), url fields
- Title priority: og:title > twitter:title > JSON-LD > `<title>` tag

**oEmbed Support**:
- Provider registry cached from oembed.com/providers.json (5-minute TTL)
- Hardcoded fallback for top 15 providers (YouTube, Vimeo, Twitter, Spotify, etc.)
- URL scheme matching via regex patterns
- Direct API calls to provider endpoints

**Wayback Fetcher** (`fetchers/wayback.rs`):
- Queries `archive.org/wayback/available?url=` for closest snapshot
- Fetches archived HTML, parses with original URL for correct relative resolution
- Strips icon URLs (rewritten by archive.org)
- Useful for pages that block scrapers but have archived copies

**Iframely Fetcher** (`fetchers/iframely.rs`):
- Queries `iframe.ly/api/iframely?url=&api_key=` for rich metadata
- Gated behind `IFRAMELY_API_KEY` env var; skipped when absent
- Extracts title, description, canonical URL, thumbnail, and icon from structured response

**DDG Fetcher** (`fetchers/ddg.rs`):
- Runs in parallel with other fetchers
- Not a post-failure fallback; contributes to merge pool with all other sources
- Provides additional title/description coverage for generic or missing content

**Configurable Fetcher Order**:
- `scrape.fetcher_order` in config.yaml controls which fetchers run and their merge priority
- Default: `[oEmbed, Wayback, Plain, Microlink, Peekalink, Iframely, DDG]`
- Remove an entry to disable that fetcher; reorder to change priority
- Headless Chrome is excluded from this list (controlled by `always_headless`)

**Smart Merge Logic**:
- Title merging: generic/auto-generated titles (site-wide defaults, error page titles, bare domains, short strings <3 chars) are discarded in favor of real titles from lower-priority sources
- Description merging: empty or very short descriptions are overridden by meaningful content from lower-priority fetchers
- Enables downstream fetchers to improve upon upstream stubs

**Image Validation Gate**:
- Magic byte detection for format verification (not Content-Type, which can lie)
- Full decode check to catch corrupt data
- Resolution check: width > 32 && height > 32
- Invalid images discarded; fallback to next priority source

**Headless Chrome Stealth**:
- Only launched when no validated image found (expensive operation)
- Additional fingerprint spoofing beyond basic User-Agent:
  - `navigator.deviceMemory` → 8
  - `navigator.maxTouchPoints` → 0
  - `WebGLRenderingContext.getParameter()` → Intel GPU vendor/renderer
  - `AudioContext` fingerprint masking
- Bypass detection for aggressive bot protections (Cloudflare, etc.)

**Error Classification** (`error.rs`):
- **Retryable**: HTTP 5xx, timeout, connection refused, DNS failure
- **Terminal**: HTTP 4xx, parse error, invalid URL, blocked host
- Task queue uses classification for retry decisions

---

## Data Flow

### Search with Semantic Ranking

```
1. CLI/API receives search query with semantic="machine learning"
2. AppService extracts filters and semantic query
3. Backend.search() applies non-semantic filters (tags, search query language, etc.)
4. If semantic query present:
   a. Ensure index reconciled (first search only)
   b. Generate query embedding
   c. Rank filtered results by cosine similarity
   d. Apply threshold filter
5. Return ranked bookmarks
```

### Bookmark Creation

```
1. Validate inputs (URL non-empty, field lengths)
2. Check for duplicate URLs
3. Backend creates bookmark (generates ID, persists to CSV)
4. If semantic enabled: embed and index (best-effort)
5. If async_meta: schedule background metadata fetch
6. Apply configured rules
```

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry point |
| `src/app/factory.rs` | Dependency injection / composition |
| `src/app/service.rs` | Business logic orchestration |
| `src/app/local.rs` | Local backend implementation |
| `src/app/remote.rs` | HTTP client backend |
| `src/bookmarks.rs` | CSV bookmark storage |
| `src/search_query/` | Search query language (lexer, parser, evaluator) |
| `src/config.rs` | Configuration loading/validation |
| `src/metadata/` | Metadata fetching pipeline (oEmbed, HTML, validation, Chrome fallback) |
| `src/workspaces.rs` | Workspace CRUD and YAML persistence |
| `src/web.rs` | HTTP API server |
| `src/cli/handlers.rs` | CLI command routing |
| `src/semantic/` | Semantic search subsystem (see below) |

---

## Semantic Search Architecture

The semantic search subsystem enables finding bookmarks by meaning rather than exact text matches.

### Module Structure

```
src/semantic/
├── mod.rs          # Public API exports
├── service.rs      # High-level service orchestration
├── embeddings.rs   # fastembed model wrapper
├── index.rs        # In-memory vector index
├── storage.rs      # Binary persistence (vectors.bin)
├── preprocess.rs   # Content preprocessing (title, desc, tags, URL)
├── lexical.rs      # Keyword matching for hybrid search
└── hybrid.rs       # RRF fusion algorithm
```

### Component Overview

```
┌─────────────────────────────────────────────────────────┐
│              SemanticSearchService                       │
│   Lazy initialization, thread-safe orchestration        │
└──┬──────────┬──────────────┬──────────────┬─────────────┘
   │          │              │              │
   ▼          ▼              ▼              ▼
┌──────────┬──────────┬──────────────┬──────────────┐
│Embedding │ Vector   │ Storage      │ Preprocess   │
│ Model    │ Index    │ (I/O)        │ (Text prep)  │
└──────────┴──────────┴──────────────┴──────────────┘

Hybrid Search (in AppService):
┌──────────────────────────────────────────────────────────┐
│                   apply_semantic_ranking()               │
│   Combines semantic + lexical rankings via RRF fusion   │
└──┬────────────────────────────────────┬─────────────────┘
   │                                    │
   ▼                                    ▼
┌──────────────────────┐    ┌──────────────────────────┐
│ Semantic Search      │    │ Lexical Scoring          │
│ (cosine similarity)  │    │ (keyword matching)       │
└──────────────────────┘    └──────────────────────────┘
           │                            │
           └────────────┬───────────────┘
                        ▼
              ┌──────────────────────┐
              │ RRF Fusion           │
              │ (rank combination)   │
              └──────────────────────┘
```

### Data Flow

**Indexing (on bookmark create/update):**
```
title, description, tags, url
    → preprocess_content()
        Format: "{title}. {title}. {description}. {tags}. {url_keywords}"
        - Title repeated for emphasis (strongest signal)
        - Tags as space-separated words (cleaner for embeddings)
        - URL keywords: domain + path segments, compounds preserved
          e.g., "github.com/rust-lang/rust-by-example"
             → "github rust lang rust-by-example example"
        - File extensions stripped (.html, .php, etc.)
        - Noise filtered (TLDs, short words, numbers)
        - Empty sections omitted, truncate to 512 chars
    → content_hash(title, desc, tags, url) (for change detection)
    → EmbeddingModel.embed() (fastembed, 384-dim vector)
    → VectorIndex.insert(id, hash, embedding)
    → VectorStorage.save() (persist to vectors.bin)
```

**Search (hybrid ranking):**
```
query text + filtered bookmarks
    ┌─────────────────────────────────────┐
    │ Semantic path:                      │
    │   → EmbeddingModel.embed(query)     │
    │   → VectorIndex.search()            │
    │   → cosine similarity ranking       │
    │   → apply threshold filter          │
    └─────────────────────────────────────┘
    ┌─────────────────────────────────────┐
    │ Lexical path:                       │
    │   → tokenize query (filter stops)   │
    │   → match against title/desc/tags   │
    │   → score: title=2x, desc=1x, tag=3x│
    └─────────────────────────────────────┘
    → RRF fusion: score(d) = 1/(k + rank_sem) + 1/(k + rank_lex)
    → merged, sorted bookmark IDs
```

### Key Components

**EmbeddingModel** (`embeddings.rs`):
- Wraps [fastembed-rs](https://github.com/Anush008/fastembed-rs) for local embedding generation
- Default model: `all-MiniLM-L6-v2` (384 dimensions, ~23MB)
- Model cached at `~/.local/share/bb/models/`

**VectorIndex** (`index.rs`):
- In-memory HashMap of bookmark ID → embedding
- Brute-force cosine similarity search (sufficient for ~1000s of bookmarks)
- Validates embedding dimensions match model

**VectorStorage** (`storage.rs`):
- Binary format with header (version, model ID, dimensions, checksum)
- Atomic writes (temp file + fsync + rename)
- Detects model changes and corrupted files

**SemanticSearchService** (`service.rs`):
- Lazy initialization (model loaded on first use)
- Index reconciliation on first search (syncs with bookmark state)
- Best-effort indexing (failures logged, don't block operations)

**LexicalScorer** (`lexical.rs`):
- Keyword matching for hybrid search
- Tokenizes query, filters stop words
- Scoring weights: title=2x, description=1x, tags=3x
- Tag matching supports hierarchical tags (`programming/rust` matches `programming`)

**HybridSearch** (`hybrid.rs`):
- Reciprocal Rank Fusion (RRF) algorithm
- Combines semantic and lexical rankings
- Formula: `score(d) = 1/(k + rank_semantic) + 1/(k + rank_lexical)`
- k=60 constant (standard from literature)
- Items appearing in both rankings get boosted

### Hybrid Search Integration

Hybrid search is always enabled when semantic search is active. The integration in `AppService::apply_semantic_ranking()`:

1. **Semantic ranking** — cosine similarity from embeddings (threshold applied)
2. **Lexical ranking** — keyword matching against title/description/tags
3. **RRF fusion** — merge both rankings, boost items appearing in both
4. **Lenient mode** — lexical matches CAN rescue items below semantic threshold

The lenient mode is important: pure tag matches may not have high semantic similarity to the query, but are still relevant. Without lenient mode, searching for "rust" would miss items tagged "rust" but without the word in title/description.

### Index Reconciliation

On first semantic search per session, the service reconciles the index:

1. **Remove orphans** — embeddings for deleted bookmarks
2. **Re-embed stale** — content hash changed since last embedding
3. **Embed missing** — bookmarks without embeddings

This ensures the index stays consistent without requiring explicit maintenance.

### Configuration

```yaml
semantic_search:
  enabled: true                    # Feature toggle
  model: "all-MiniLM-L6-v2"       # Embedding model
  default_threshold: 0.35          # Min similarity (0.0-1.0)
  embedding_parallelism: 4         # Concurrent embeddings
  download_timeout_secs: 300       # Model download timeout
```

### Storage Format

`vectors.bin` structure:
```
Header (47 bytes):
  version: u8
  model_id: [u8; 32]     # SHA256(model_name)
  dimensions: u16
  entry_count: u64
  checksum: u32          # CRC32

Entries:
  id: u64
  content_hash: u64
  embedding: [f32; dimensions]
```

### Error Handling

Semantic search uses best-effort semantics:
- **Indexing failures** — logged as warnings, don't fail bookmark operations
- **Search failures** — propagated to caller with typed errors
- **Model unavailable** — returns HTTP 503 / CLI error

Error types: `SemanticDisabled`, `InvalidThreshold`, `ModelUnavailable`, `Embedding(...)`, `Storage(...)`

---

## Thread Safety

- `Arc<RwLock<Config>>` — shared configuration (read-only at runtime)
- `Arc<RwLock<RulesConfig>>` — shared rules (read/write)
- `Arc<RwLock<WorkspaceStore>>` — workspace persistence
- `Arc<dyn BookmarkManager>` — cloned per worker
- `Mutex<Option<SemanticState>>` — lazy-loaded semantic state
- `AtomicBool` — reconciliation flag (ensures single execution)

---

## Testing

Tests located in `src/tests/`:

| File | Purpose |
|------|---------|
| `app.rs` | AppLocal CRUD and search |
| `bookmarks.rs` | Search query |
| `search_query/tests.rs` | Query language parsing and evaluation |
| `rules.rs` | Rule matching |
| `semantic.rs` | Semantic search (1800+ lines) |

Semantic tests requiring model download are marked `#[ignore]`:
```bash
# Fast unit tests
cargo test

# Semantic integration tests (downloads ~23MB model)
cargo test -- --ignored
```
