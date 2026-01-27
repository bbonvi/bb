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

### 3. Configuration (`src/config.rs`)

YAML config at `~/.local/share/bb/config.yaml`:
```yaml
task_queue_max_threads: 4
hidden_by_default: []
rules: []
semantic_search:
  enabled: false
  model: "all-MiniLM-L6-v2"
  default_threshold: 0.35
```

Config validates on load; invalid config panics early.

### 4. Workspace Storage (`src/workspaces.rs`)

YAML-persisted workspace definitions at `~/.local/share/bb/workspaces.yaml`:
- `WorkspaceStore` holds `Vec<Workspace>` in `Arc<RwLock<>>` (same concurrency pattern as config)
- Atomic writes via `BackendLocal::write`
- Auto-creates empty file on first load
- Validation: name non-empty/trimmed/max 100 chars, regex patterns must compile, no duplicate names (case-insensitive)
- ID generation via `Eid` (ULID-based)
- Workspace filtering is frontend-only; the backend stores filter definitions but does not evaluate them

### 5. CLI Layer (`src/cli/`)

**handlers.rs** — Entry points called from `main.rs`:
- `handle_search()`, `handle_add()`, `handle_rule()`, etc.

**commands.rs** — Encapsulated command execution:
- `SearchCommand`, `AddCommand`, `MetaCommand`, `RuleCommand`
- Each validates inputs, executes via AppService, formats output

**validation.rs** — Input validation:
- Search query requires at least one criterion
- Semantic threshold in [0.0, 1.0]
- Tag format validation (no spaces, max length)

### 6. Web/API Layer (`src/web.rs`)

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

### 7. Task Queue (`src/app/task_runner.rs`)

Background metadata fetching for `--async-meta` flag:
- Tasks persisted to `task-queue.json` for recovery
- Worker thread pool with configurable concurrency
- Graceful shutdown on SIGTERM

---

## Data Flow

### Search with Semantic Ranking

```
1. CLI/API receives search query with semantic="machine learning"
2. AppService extracts filters and semantic query
3. Backend.search() applies non-semantic filters (tags, keywords, etc.)
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
| `src/config.rs` | Configuration loading/validation |
| `src/workspaces.rs` | Workspace CRUD and YAML persistence |
| `src/web.rs` | HTTP API server |
| `src/cli/handlers.rs` | CLI command routing |
| `src/semantic/` | Semantic search subsystem (see below) |

---

## Semantic Search Architecture

The semantic search subsystem enables finding bookmarks by meaning rather than exact keywords.

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

- `Arc<RwLock<Config>>` — shared configuration
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
| `bookmarks.rs` | Keyword search |
| `rules.rs` | Rule matching |
| `semantic.rs` | Semantic search (1800+ lines) |

Semantic tests requiring model download are marked `#[ignore]`:
```bash
# Fast unit tests
cargo test

# Semantic integration tests (downloads ~23MB model)
cargo test -- --ignored
```
