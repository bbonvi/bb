// Workspace filter evaluation logic (§6.3)
//
// Hybrid strategy: plain tags/substrings are injected into the search query
// (server-side), while glob patterns, regex, and blacklist are applied
// client-side after fetch.

import type { Bookmark, SearchQuery, Workspace } from './api'

// --- Glob matching ---
// Converts a glob pattern to a RegExp. Supports *, ?, and [...] character classes.
function globToRegex(pattern: string): RegExp {
  let re = '^'
  for (let i = 0; i < pattern.length; i++) {
    const c = pattern[i]
    if (c === '*') re += '.*'
    else if (c === '?') re += '.'
    else if (c === '[') {
      // Pass through character class until ]
      const start = i
      i++
      if (i < pattern.length && pattern[i] === '!') {
        re += '[^'
        i++
      } else {
        re += '['
      }
      while (i < pattern.length && pattern[i] !== ']') {
        re += pattern[i]
        i++
      }
      if (i < pattern.length) re += ']'
      else re += pattern.slice(start) // malformed — literal
    } else {
      re += pattern.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').slice(0, 0) // no-op
      // Escape special regex chars
      re += c.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    }
  }
  re += '$'
  return new RegExp(re, 'i')
}

function hasGlobChars(s: string): boolean {
  return /[*?\[]/.test(s)
}

function hasRegexChars(s: string): boolean {
  return /[.*+?^${}()|\\[\]]/.test(s)
}

// --- Server-side query injection ---
// Modifies the search query to include plain workspace filters that can be
// delegated to the backend.
export function injectWorkspaceFilters(
  query: SearchQuery,
  workspace: Workspace,
): SearchQuery {
  const injected = { ...query }
  const f = workspace.filters

  // Plain tags (no globs) → append to tags field
  const plainTags = f.tag_whitelist.filter((t) => !hasGlobChars(t))
  if (plainTags.length > 0) {
    const existing = injected.tags ? injected.tags.split(',').map((t) => t.trim()).filter(Boolean) : []
    const merged = [...new Set([...existing, ...plainTags])]
    injected.tags = merged.join(',')
  }

  // Simple substring patterns → inject into corresponding search fields
  if (f.title_pattern && !hasRegexChars(f.title_pattern) && !injected.title) {
    injected.title = f.title_pattern
  }
  if (f.url_pattern && !hasRegexChars(f.url_pattern) && !injected.url) {
    injected.url = f.url_pattern
  }
  if (f.description_pattern && !hasRegexChars(f.description_pattern) && !injected.description) {
    injected.description = f.description_pattern
  }

  return injected
}

// --- Client-side post-filter ---
// Applies workspace filters that can't be delegated to the backend.
export function applyWorkspaceFilter(
  bookmarks: Bookmark[],
  workspace: Workspace,
): Bookmark[] {
  const f = workspace.filters

  // Glob tag whitelist (only patterns with glob chars — plain tags already sent server-side)
  const globWhitelist = f.tag_whitelist.filter(hasGlobChars).map(globToRegex)

  // Regex patterns (only when they contain regex metacharacters — simple strings already sent server-side)
  const titleRe = f.title_pattern && hasRegexChars(f.title_pattern) ? safeRegex(f.title_pattern) : null
  const urlRe = f.url_pattern && hasRegexChars(f.url_pattern) ? safeRegex(f.url_pattern) : null
  const descRe = f.description_pattern && hasRegexChars(f.description_pattern) ? safeRegex(f.description_pattern) : null
  const anyRe = f.any_field_pattern ? safeRegex(f.any_field_pattern) : null

  // Blacklist glob patterns
  const blacklistPatterns = f.tag_blacklist.map(globToRegex)

  const needsClientInclusion = globWhitelist.length > 0 || titleRe || urlRe || descRe || anyRe
  const needsBlacklist = blacklistPatterns.length > 0

  if (!needsClientInclusion && !needsBlacklist) return bookmarks

  return bookmarks.filter((bm) => {
    // Inclusion: if there are client-side inclusion filters, bookmark must match at least one
    if (needsClientInclusion) {
      const matchesGlob = globWhitelist.length > 0 && bm.tags.some((t) => globWhitelist.some((re) => re.test(t)))
      const matchesTitle = titleRe ? titleRe.test(bm.title) : false
      const matchesUrl = urlRe ? urlRe.test(bm.url) : false
      const matchesDesc = descRe ? descRe.test(bm.description) : false
      const matchesAny = anyRe
        ? anyRe.test(bm.title) || anyRe.test(bm.url) || anyRe.test(bm.description) || bm.tags.some((t) => anyRe.test(t))
        : false

      if (!matchesGlob && !matchesTitle && !matchesUrl && !matchesDesc && !matchesAny) {
        return false
      }
    }

    // Exclusion: blacklisted tag match → exclude
    if (needsBlacklist) {
      if (bm.tags.some((t) => blacklistPatterns.some((re) => re.test(t)))) {
        return false
      }
    }

    return true
  })
}

// --- Uncategorized filter ---
// Returns bookmarks whose tags don't match ANY workspace's whitelist.
export function filterUncategorized(
  bookmarks: Bookmark[],
  allWorkspaces: Workspace[],
): Bookmark[] {
  // Collect all whitelist patterns from all workspaces
  const allPatterns = allWorkspaces.flatMap((ws) =>
    ws.filters.tag_whitelist.map((p) => (hasGlobChars(p) ? globToRegex(p) : new RegExp(`^${p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`, 'i'))),
  )

  if (allPatterns.length === 0) return bookmarks

  return bookmarks.filter(
    (bm) => !bm.tags.some((t) => allPatterns.some((re) => re.test(t))),
  )
}

// Safe regex construction — returns null on invalid pattern
function safeRegex(pattern: string): RegExp | null {
  try {
    return new RegExp(pattern, 'i')
  } catch {
    return null
  }
}
