import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('DialogContent', () => {
  test('constrains overflowing content on small screens', () => {
    const source = readFileSync(new URL('../DialogContent.vue', import.meta.url), 'utf8')

    expect(source).toContain('max-h-[calc(100dvh-2rem)]')
    expect(source).toContain('overflow-y-auto')
  })

  test('localizes the close control text', () => {
    const source = readFileSync(new URL('../DialogContent.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(source).toContain('const { t } = useI18n()')
    expect(source).toContain("{{ t('common.close') }}")
    expect(source).not.toContain('>关闭<')
  })
})
