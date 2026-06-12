import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

const tableViews = [
  '../admin/AdminAuditView.vue',
  '../admin/AdminCredentialsView.vue',
  '../admin/AdminDevicesView.vue',
  '../admin/AdminOAuthView.vue',
  '../admin/AdminPlansView.vue',
  '../admin/AdminRelaysView.vue',
  '../admin/AdminSessionsView.vue',
  '../admin/AdminUsersView.vue',
  '../user/UserAccountView.vue',
  '../user/UserControllersView.vue',
  '../user/UserCredentialsView.vue',
  '../user/UserDevicesView.vue',
]

describe('route state components', () => {
  test('data list views use ResponsiveTable for loading, error, empty, and retry states', () => {
    for (const view of tableViews) {
      const source = readFileSync(new URL(view, import.meta.url), 'utf8')

      expect(source, view).toContain("import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'")
      expect(source, view).toContain('<ResponsiveTable')
      expect(source, view).toContain(':loading=')
      expect(source, view).toContain(':error=')
      expect(source, view).toContain(':empty-title=')
      expect(source, view).toContain('@retry=')
    }
  })

  test('data list views provide mobile card fallbacks', () => {
    for (const view of tableViews) {
      const source = readFileSync(new URL(view, import.meta.url), 'utf8')

      expect(source, view).toContain('<template #cards>')
    }
  })

  test('sensitive token result blocks constrain long content on small screens', () => {
    const tokenViews = [
      '../admin/AdminCredentialsView.vue',
      '../user/UserCredentialsView.vue',
      '../user/UserDevicesView.vue',
    ]

    for (const view of tokenViews) {
      const source = readFileSync(new URL(view, import.meta.url), 'utf8')

      expect(source, view).toContain('max-h-28 overflow-auto break-all rounded-md bg-muted p-3 font-mono text-xs')
    }
  })

  test('dashboard views use shared loading and error states', () => {
    for (const view of ['../admin/AdminDashboardView.vue', '../user/UserDashboardView.vue']) {
      const source = readFileSync(new URL(view, import.meta.url), 'utf8')

      expect(source, view).toContain("import LoadingState from '@/components/layout/LoadingState.vue'")
      expect(source, view).toContain("import ErrorState from '@/components/layout/ErrorState.vue'")
      expect(source, view).toContain('<LoadingState')
      expect(source, view).toContain('<ErrorState')
    }
  })
})
