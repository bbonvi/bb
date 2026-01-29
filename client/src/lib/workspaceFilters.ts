// Workspace filter → keyword query construction
//
// Translates workspace filter fields (tag whitelist, blacklist, keyword)
// into a keyword search query string for server-side evaluation.

import type { SearchQuery, Workspace } from './api'

/**
 * Build a keyword query string from workspace filter fields.
 * Returns null if no filters produce any query terms.
 */
export function buildWorkspaceKeyword(workspace: Workspace): string | null {
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

  // Keyword: append as-is
  if (f.keyword) {
    parts.push(f.keyword)
  }

  if (parts.length === 0) return null
  return parts.join(' ')
}

/**
 * Merge workspace keyword query with user's search query.
 * The workspace keyword is AND-combined with the user's existing keyword.
 */
export function mergeWorkspaceQuery(
  userQuery: SearchQuery,
  workspace: Workspace,
): SearchQuery {
  const wsKeyword = buildWorkspaceKeyword(workspace)
  if (!wsKeyword) return userQuery

  const merged = { ...userQuery }
  if (merged.keyword && merged.keyword.trim()) {
    merged.keyword = `(${wsKeyword}) (${merged.keyword})`
  } else {
    merged.keyword = wsKeyword
  }
  return merged
}
