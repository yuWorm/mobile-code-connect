import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('SearchToolbar', () => {
  test('labels search input and refresh action for assistive technology', () => {
    const source = readFileSync(new URL('../SearchToolbar.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(source).toContain('const { t } = useI18n()')
    expect(source).toContain('searchLabel?: string')
    expect(source).toContain('refreshLabel?: string')
    expect(source).toContain('<Search class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />')
    expect(source).toContain(":placeholder=\"placeholder ?? t('common.search')\"")
    expect(source).toContain(":aria-label=\"searchLabel ?? placeholder ?? t('common.search')\"")
    expect(source).toContain('<Button type="button" variant="outline"')
    expect(source).toContain(":aria-label=\"refreshLabel ?? t('common.refreshList')\"")
    expect(source).toContain('<RefreshCw :class="[\'size-4\', loading ? \'animate-spin\' : \'\']" aria-hidden="true" />')
    expect(source).toContain("{{ t('common.refresh') }}")
    expect(source).not.toContain("'搜索'")
    expect(source).not.toContain("'刷新列表'")
    expect(source).not.toContain('>刷新<')
  })
})
