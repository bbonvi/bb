# Command-Line Help for `bb`

This document contains the help content for the `bb` command-line program.

## Authentication

When `BB_AUTH_TOKEN` is set on the daemon, all API endpoints require authentication via the `Authorization` header:

```
Authorization: Bearer <token>
```

**Example:**
```bash
curl -H "Authorization: Bearer your-secret-token" http://localhost:8080/api/config
```

When `BB_AUTH_TOKEN` is unset or empty, authentication is disabled and all requests pass through.

---

## HTTP API

### `POST /api/bookmarks/search`

Search for bookmarks with optional semantic ranking.

**Request Body:**
```json
{
  "id": 123,
  "url": "example.com",
  "title": "search term",
  "description": "search term",
  "tags": "tag1,tag2",
  "query": "general search",
  "exact": false,
  "semantic": "find by meaning",
  "threshold": 0.35,
  "limit": 50
}
```

All fields are optional. When `semantic` is provided:
- Results are ranked by semantic similarity to the query
- `threshold` filters results below the similarity score (0.0-1.0, default: 0.35)
- Semantic ranking applies after other filters (url, title, tags, etc.)

**Response:**
```json
[
  {
    "id": 1,
    "url": "https://example.com",
    "title": "Example",
    "description": "An example bookmark",
    "tags": ["example", "test"],
    "image_id": "abc123.png",
    "icon_id": "def456.png"
  }
]
```

**Error Responses:**

| Status | Error Code | Description |
|--------|------------|-------------|
| 400 | `INVALID_THRESHOLD` | Threshold outside valid range (0.0-1.0) |
| 422 | `SEMANTIC_DISABLED` | Semantic search requested but disabled in config |
| 503 | `MODEL_UNAVAILABLE` | Embedding model failed to load |

### `GET /api/semantic/status`

Check semantic search feature status.

**Response:**
```json
{
  "enabled": true,
  "model": "all-MiniLM-L6-v2",
  "indexed_count": 150,
  "total_bookmarks": 200
}
```

| Field | Description |
|-------|-------------|
| `enabled` | Whether semantic search is enabled in config |
| `model` | Configured embedding model name |
| `indexed_count` | Number of bookmarks with embeddings |
| `total_bookmarks` | Total bookmark count |

### `GET /api/workspaces`

List all workspaces.

**Response:**
```json
[
  {
    "id": "01JJXYZ...",
    "name": "Dev",
    "filters": {
      "tag_whitelist": ["dev", "rust"],
      "tag_blacklist": ["internal"],
      "query": null
    },
    "view_prefs": {
      "mode": "grid",
      "columns": 3
    }
  }
]
```

### `POST /api/workspaces`

Create a new workspace.

**Request Body:**
```json
{
  "name": "Dev",
  "filters": {
    "tag_whitelist": ["dev"],
    "tag_blacklist": [],
    "query": ":example.dev"
  },
  "view_prefs": {
    "mode": "grid",
    "columns": 3
  }
}
```

Only `name` is required. `filters` and `view_prefs` default to empty/null values.

**Response (201):**
Returns the created workspace object (same shape as list items).

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 400 | Empty/whitespace name, name > 100 chars, invalid query, duplicate name (case-insensitive) |

### `PUT /api/workspaces/:id`

Update an existing workspace. All fields are optional (partial update).

**Request Body:**
```json
{
  "name": "Renamed",
  "filters": { "tag_whitelist": ["new-tag"] },
  "view_prefs": { "columns": 4 }
}
```

**Response (200):** Updated workspace object.

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 400 | Same validation as create |
| 404 | Workspace ID not found |

### `DELETE /api/workspaces/:id`

Delete a workspace.

**Response:** 204 No Content.

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 404 | Workspace ID not found |

---

**Command Overview:**

* [`bb`↴](#bb)
* [`bb daemon`↴](#bb-daemon)
* [`bb search`↴](#bb-search)
* [`bb search update`↴](#bb-search-update)
* [`bb search delete`↴](#bb-search-delete)
* [`bb add`↴](#bb-add)
* [`bb meta`↴](#bb-meta)
* [`bb rule`↴](#bb-rule)
* [`bb rule add`↴](#bb-rule-add)
* [`bb rule add update`↴](#bb-rule-add-update)
* [`bb rule list`↴](#bb-rule-list)
* [`bb rule delete`↴](#bb-rule-delete)

## `bb`

**Usage:** `bb <COMMAND>`

###### **Subcommands:**

* `daemon` — Start bb as a service
* `search` — Search bookmark
* `add` — 
* `meta` — Query website meta data
* `rule` — Manage automated rules



## `bb daemon`

Start bb as a service

**Usage:** `bb daemon`



## `bb search`

Search bookmark

**Usage:** `bb search [OPTIONS] [COMMAND]`

###### **Subcommands:**

* `update` — Update found bookmarks
* `delete` — Delete found bookmarks

###### **Options:**

* `-u`, `--url <URL>` — a url
* `-t`, `--title <TITLE>` — Bookmark title
* `-d`, `--description <DESCRIPTION>` — Bookmark description
* `-g`, `--tags <TAGS>` — Bookmark tags
* `-k`, `--query <QUERY>` — Search query across title, description, url, and tags (use #tag for tag search)
* `-i`, `--id <ID>` — id
* `-e`, `--exact` — Exact search. False by default

  Default value: `false`
* `-s`, `--sem <SEMANTIC>` — Semantic search query (find bookmarks by meaning)
* `--threshold <THRESHOLD>` — Similarity threshold for semantic search (0.0-1.0)
* `-c`, `--count` — Print the count

  Default value: `false`



## `bb search update`

Update found bookmarks

**Usage:** `bb search update [OPTIONS]`

###### **Options:**

* `-u`, `--url <URL>` — a url
* `-t`, `--title <TITLE>` — Bookmark title
* `-d`, `--description <DESCRIPTION>` — Bookmark description
* `--tags <TAGS>` — Replace tags
* `-a`, `--append-tags <APPEND_TAGS>` — Appends tags
* `-r`, `--remove-tags <REMOVE_TAGS>` — Delete tags



## `bb search delete`

Delete found bookmarks

**Usage:** `bb search delete [OPTIONS]`

###### **Options:**

* `-y`, `--yes` — Auto confirm

  Default value: `false`
* `-f`, `--force` — Don't ask for confirmation when performing dangerous delete. (e.g. when attempting to delete all bookmarks)

  Default value: `false`



## `bb add`

**Usage:** `bb add [OPTIONS]`

###### **Arguments:**

* `<URL>` — a url

###### **Options:**

* `--editor`

  Default value: `false`
* `-t`, `--title <TITLE>` — Bookmark title
* `-d`, `--description <DESCRIPTION>` — Bookmark description
* `-g`, `--tags <TAGS>` — Bookmark tags
* `--async-meta` — fetch metadata in background (only when used as client)

  Default value: `false`
* `--no-https-upgrade` — Don't try to upgrade to https

  Default value: `false`
* `--no-headless` — Don't use headless browser to capture screenshots and metadata

  Default value: `false`
* `--no-meta` — Don't fetch meta at all

  Default value: `false`



## `bb meta`

Query website meta data

**Usage:** `bb meta [OPTIONS] <URL>`

###### **Arguments:**

* `<URL>` — A url

###### **Options:**

* `--no-https-upgrade` — Don't try to upgrade to https

  Default value: `false`
* `--no-headless` — Don't use headless browser to capture screenshots and metadata

  Default value: `false`
* `--no-meta` — Don't fetch meta at all

  Default value: `false`



## `bb rule`

Manage automated rules

**Usage:** `bb rule <COMMAND>`

###### **Subcommands:**

* `add` — Create new rule
* `list` — List all rules
* `delete` — UNIMPLEMENTED! Edit config.yaml directly



## `bb rule add`

Create new rule

**Usage:** `bb rule add [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `update` — Perform an Update action

###### **Options:**

* `--url <URL>` — A regex matching bookmark url
* `--title <TITLE>` — A regex matching bookmark title
* `--description <DESCRIPTION>` — A regex matching bookmark description
* `--tags <TAGS>` — A list of tags bookmark will be matched by (all tags has to match)



## `bb rule add update`

Perform an Update action

**Usage:** `bb rule add update [OPTIONS]`

###### **Options:**

* `--title <TITLE>` — Set bookmark title
* `--description <DESCRIPTION>` — Set bookmark description
* `--tags <TAGS>` — Add tags



## `bb rule list`

List all rules

**Usage:** `bb rule list`



## `bb rule delete`

UNIMPLEMENTED! Edit config.yaml directly

**Usage:** `bb rule delete`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

