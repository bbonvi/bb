import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import ts from 'typescript'

const source = await readFile(new URL('../src/lib/navigationState.ts', import.meta.url), 'utf8')
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
})
const navigationState = await import(
  `data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`
)

test('standalone launch restores pinned state instead of its fixed URL', () => {
  const pinnedState = {
    searchQuery: { query: 'saved search' },
    showAll: false,
    activeWorkspaceId: 'saved-workspace',
  }

  assert.deepEqual(
    navigationState.resolveInitialNavigationState({
      search: '?query=install-time-search&all=1&workspace=Install-time',
      standalone: true,
      pinnedState,
      lastWorkspaceId: 'last-workspace',
    }),
    {
      ...pinnedState,
      urlWorkspaceName: null,
    },
  )
})
