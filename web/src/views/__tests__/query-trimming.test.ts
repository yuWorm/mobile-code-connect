import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

const backendSearchQueries = [
  ['../admin/AdminAuditView.vue', 'q: q.value.trim()'],
  ['../admin/AdminCredentialsView.vue', 'q: q.value.trim()'],
  ['../admin/AdminDevicesView.vue', 'q: q.value.trim()'],
  ['../admin/AdminDevicesView.vue', 'q: accessQ.value.trim()'],
  ['../admin/AdminOAuthView.vue', 'q: q.value.trim()'],
  ['../admin/AdminPlansView.vue', 'q: q.value.trim()'],
  ['../admin/AdminRelaysView.vue', 'q: q.value.trim()'],
  ['../admin/AdminRelaysView.vue', 'q: credentialQ.value.trim()'],
  ['../admin/AdminSessionsView.vue', 'q: q.value.trim()'],
  ['../admin/AdminUsersView.vue', 'q: q.value.trim()'],
  ['../admin/AdminUsersView.vue', 'q: usageQ.value.trim()'],
  ['../user/UserAccountView.vue', 'q: q.value.trim()'],
  ['../user/UserControllersView.vue', 'q: q.value.trim()'],
  ['../user/UserCredentialsView.vue', 'q: q.value.trim()'],
] as const

describe('backend list queries', () => {
  test('trim free-text search before sending requests', () => {
    for (const [view, expected] of backendSearchQueries) {
      const source = readFileSync(new URL(view, import.meta.url), 'utf8')

      expect(source).toContain(expected)
    }
  })
})
