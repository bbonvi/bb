// Workspace filter → search query construction
//
// Translates workspace filter fields (tag whitelist, blacklist, query)
// into a search query string for server-side evaluation.

import type { SearchQuery, Workspace } from './api'

/**
 * Build a search query string from workspace filter fields.
 * Returns null if no filters produce any query terms.
 */
export function buildWorkspaceQuery(workspace: Workspace): string | null {
  const parts: string[] = []
  const f = workspace.filters

  // Tag whitelist: OR semantics → (#tag1 or #tag2 or ...)
  if (f.tag_whitelist.length > 0) {
    if (f.tag_whitelist.length === 1) {
      parts.push(`#${f.tag_whitelist[0]}`)
    } else {
      const tags = f.tag_whitelist.map((t) => `#${t}`).join(' or ')
      parts.push(`(${tags})`)
    }
  }

  // Tag blacklist: each negated independently → not #tag1 not #tag2
  for (const tag of f.tag_blacklist) {
    parts.push(`not #${tag}`)
  }

  // Query: append as-is
  if (f.query) {
    parts.push(f.query)
  }

  if (parts.length === 0) return null
  return parts.join(' ')
}

/**
 * Merge workspace query with user's search query.
 * The workspace query is AND-combined with the user's existing query.
 */
export function mergeWorkspaceQuery(
  userQuery: SearchQuery,
  workspace: Workspace,
): SearchQuery {
  const wsKeyword = buildWorkspaceQuery(workspace)
  if (!wsKeyword) return userQuery

  const merged = { ...userQuery }
  if (merged.query && merged.query.trim()) {
    merged.query = `(${wsKeyword}) (${merged.query})`
  } else {
    merged.query = wsKeyword
  }
  return merged
}
