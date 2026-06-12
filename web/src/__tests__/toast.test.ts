import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('toast integration', () => {
  test('mounts vue-sonner toaster in the app root', () => {
    const source = readFileSync(new URL('../App.vue', import.meta.url), 'utf8')

    expect(source).toContain("import 'vue-sonner/style.css'")
    expect(source).toContain('Toaster')
  })

  test('copy flows show success feedback', () => {
    const devices = readFileSync(new URL('../views/user/UserDevicesView.vue', import.meta.url), 'utf8')
    const adminCredentials = readFileSync(
      new URL('../views/admin/AdminCredentialsView.vue', import.meta.url),
      'utf8',
    )
    const credentials = readFileSync(
      new URL('../views/user/UserCredentialsView.vue', import.meta.url),
      'utf8',
    )
    const clipboard = readFileSync(new URL('../lib/control/clipboard.ts', import.meta.url), 'utf8')

    expect(devices).toContain('copyToClipboard')
    expect(adminCredentials).toContain('copyToClipboard')
    expect(credentials).toContain('copyToClipboard')
    expect(clipboard).toContain('toast.success(success)')
    expect(clipboard).toContain('toast.error(')
  })
})
