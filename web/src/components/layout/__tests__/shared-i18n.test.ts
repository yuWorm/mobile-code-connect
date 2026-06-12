import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('shared layout i18n defaults', () => {
  test('loading, error, and responsive table defaults use translations', () => {
    const loading = readFileSync(new URL('../LoadingState.vue', import.meta.url), 'utf8')
    const error = readFileSync(new URL('../ErrorState.vue', import.meta.url), 'utf8')
    const table = readFileSync(new URL('../ResponsiveTable.vue', import.meta.url), 'utf8')

    for (const source of [loading, error, table]) {
      expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
      expect(source).toContain('const { t } = useI18n()')
    }

    expect(loading).toContain("{{ label ?? t('common.loading') }}")
    expect(loading).not.toContain("'加载中'")

    expect(error).toContain("{{ t('common.retry') }}")
    expect(error).not.toContain('重试')

    expect(table).toContain(":title=\"emptyTitle ?? t('common.emptyData')\"")
    expect(table).not.toContain("'暂无数据'")
  })

  test('shared loading, error, and empty states expose accessible status semantics', () => {
    const loading = readFileSync(new URL('../LoadingState.vue', import.meta.url), 'utf8')
    const error = readFileSync(new URL('../ErrorState.vue', import.meta.url), 'utf8')
    const empty = readFileSync(new URL('../EmptyState.vue', import.meta.url), 'utf8')

    expect(loading).toContain('role="status"')
    expect(loading).toContain('aria-live="polite"')
    expect(loading).toContain('<Loader2 class="size-4 animate-spin" aria-hidden="true" />')

    expect(error).toContain('role="alert"')
    expect(error).toContain('aria-live="assertive"')
    expect(error).toContain('<RefreshCw class="size-4" aria-hidden="true" />')
    expect(error).toContain('<Button type="button" variant="outline" size="sm" @click="$emit(\'retry\')">')

    expect(empty).toContain('role="status"')
    expect(empty).toContain('aria-live="polite"')
    expect(empty).toContain('aria-hidden="true"')
  })
})
