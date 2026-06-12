import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('useAuth source contract', () => {
  test('registers a global API auth failure handler that clears local session state', () => {
    const source = readFileSync(new URL('../useAuth.ts', import.meta.url), 'utf8')

    expect(source).toContain('setControlApiAuthFailureHandler(handleAuthFailure)')
    expect(source).toContain('function handleAuthFailure()')
    expect(source).toContain('clearStoredSession()')
    expect(source).toContain('setControlApiToken(null)')
    expect(source).toContain('state.session = null')
    expect(source).toContain("router.replace({ path: '/login', query: { redirect: currentRoute.fullPath } })")
  })
})
