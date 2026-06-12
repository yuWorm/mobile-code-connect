import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('ByteUnitInput', () => {
  test('edits backend byte values through KB MB GB TB controls', () => {
    const source = readFileSync(new URL('../ByteUnitInput.vue', import.meta.url), 'utf8')

    expect(source).toContain('byteUnitOptions')
    expect(source).toContain('bytesToUnitInput')
    expect(source).toContain('unitInputToBytes')
    expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(source).toContain('const { t } = useI18n()')
    expect(source).toContain('const syncingFromModel = ref(false)')
    expect(source).toContain('if (syncingFromModel.value) {')
    expect(source).toContain('const nextBytes = unitInputToBytes(amount.value, unit.value)')
    expect(source).toContain("emit('update:modelValue', nextBytes)")
    expect(source).toContain('<Input')
    expect(source).toContain('<Select v-model="unit"')
    expect(source).toContain('<SelectItem')
    expect(source).toContain('option.label')
    expect(source).toContain(":aria-label=\"t('common.unitFor', { label })\"")
    expect(source).toContain(":placeholder=\"t('common.unit')\"")
    expect(source).not.toContain('`${label}单位`')
    expect(source).not.toContain('placeholder="单位"')
    expect(source).toContain('{{ rate ? \'/s\' : \'\' }}')
  })
})
