import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('vite control api proxy', () => {
  test('proxies relay bootstrap endpoints during local development', () => {
    const source = readFileSync(new URL('../../vite.config.ts', import.meta.url), 'utf8')

    expect(source).toContain("'/relay-bootstraps'")
  })
})
