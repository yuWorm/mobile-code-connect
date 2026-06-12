import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('DurationUnitInput', () => {
  test('edits backend seconds through minute hour day controls', () => {
    const source = readFileSync(new URL('../DurationUnitInput.vue', import.meta.url), 'utf8')

    expect(source).toContain('durationUnitOptions')
    expect(source).toContain('durationToUnitInput')
    expect(source).toContain('unitInputToDurationSeconds')
    expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(source).toContain('const { t } = useI18n()')
    expect(source).toContain('const syncingFromModel = ref(false)')
    expect(source).toContain('const nextSeconds = unitInputToDurationSeconds(amount.value, unit.value)')
    expect(source).toContain("emit('update:modelValue', nextSeconds)")
    expect(source).toContain('<Input')
    expect(source).toContain('<Select v-model="unit"')
    expect(source).toContain('option.labelKey')
    expect(source).toContain('{{ t(option.labelKey) }}')
    expect(source).toContain(":aria-label=\"t('common.unitFor', { label })\"")
    expect(source).toContain(":placeholder=\"t('common.unit')\"")
    expect(source).not.toContain('`${label}单位`')
    expect(source).not.toContain('placeholder="单位"')
  })
})
