import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('ConfirmAction', () => {
  test('uses the existing dialog trigger and exposes a confirm event', () => {
    const source = readFileSync(new URL('../ConfirmAction.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(source).toContain('const { t } = useI18n()')
    expect(source).toContain('DialogTrigger as-child')
    expect(source).toContain('defineEmits<{ confirm: [] }>()')
    expect(source).toContain("variant: 'destructive'")
    expect(source).toContain('loading?: boolean')
    expect(source).toContain('action?: () => MaybePromise<void>')
    expect(source).toContain('const internalLoading = ref(false)')
    expect(source).toContain('await props.action()')
    expect(source).toContain('Loader2')
    expect(source).toContain('<Button type="button" :class="props.class"')
    expect(source).toContain('<component :is="icon" v-if="icon" class="size-4" aria-hidden="true" />')
    expect(source).toContain('<Button type="button" variant="outline" :disabled="isLoading" @click="cancel">')
    expect(source).toContain('<Button type="button" variant="destructive" :disabled="isLoading" @click="confirm">')
    expect(source).toContain('<Loader2 v-if="isLoading" class="animate-spin" aria-hidden="true" />')
    expect(source).toContain("{{ cancelText ?? t('common.cancel') }}")
    expect(source).toContain("{{ confirmText ?? t('common.confirm') }}")
    expect(source).not.toContain("confirmText: '确认'")
    expect(source).not.toContain("cancelText: '取消'")
  })
})
