import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

const mutationViews = [
  '../admin/AdminDevicesView.vue',
  '../admin/AdminPlansView.vue',
  '../admin/AdminRelaysView.vue',
  '../admin/AdminSessionsView.vue',
  '../admin/AdminUsersView.vue',
  '../user/UserAccountView.vue',
  '../user/UserControllersView.vue',
  '../user/UserCredentialsView.vue',
  '../user/UserDevicesView.vue',
]

const busyActionViews = [
  '../admin/AdminCredentialsView.vue',
  '../admin/AdminDevicesView.vue',
  '../admin/AdminOAuthView.vue',
  '../admin/AdminRelaysView.vue',
  '../admin/AdminSessionsView.vue',
  '../admin/AdminUsersView.vue',
  '../user/UserAccountView.vue',
  '../user/UserControllersView.vue',
  '../user/UserCredentialsView.vue',
]

describe('mutation feedback', () => {
  test('high-risk mutation views use shared toast feedback', () => {
    for (const view of mutationViews) {
      const source = readFileSync(new URL(view, import.meta.url), 'utf8')
      expect(source).toContain('runWithToast')
    }
  })

  test('confirmation actions expose loading state for async mutations', () => {
    const sessions = readFileSync(new URL('../admin/AdminSessionsView.vue', import.meta.url), 'utf8')
    const devices = readFileSync(new URL('../admin/AdminDevicesView.vue', import.meta.url), 'utf8')

    expect(sessions).toContain(':loading="isBusy')
    expect(devices).toContain(':loading="isBusy')
  })

  test('row mutation views use an exclusive action guard before calling APIs', () => {
    for (const view of busyActionViews) {
      const source = readFileSync(new URL(view, import.meta.url), 'utf8')

      expect(source).toContain("import { useBusyAction } from '@/composables/useBusyAction'")
      expect(source).toContain('runBusyAction(')
      expect(source).toContain('hasBusyAction')
      expect(source).toContain(':disabled="hasBusyAction')
    }
  })
})
