import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

import {
  EMPTY_SELECT_ITEM_VALUE,
  fromSelectInternalValue,
  toSelectInternalItemValue,
} from '../select/empty-value'

describe('Select empty string value handling', () => {
  test('maps empty-string items to a non-empty internal value', () => {
    expect(EMPTY_SELECT_ITEM_VALUE).not.toBe('')
    expect(toSelectInternalItemValue('')).toBe(EMPTY_SELECT_ITEM_VALUE)
    expect(toSelectInternalItemValue('enabled')).toBe('enabled')
    expect(fromSelectInternalValue(EMPTY_SELECT_ITEM_VALUE)).toBe('')
    expect(fromSelectInternalValue('enabled')).toBe('enabled')
  })

  test('select wrappers apply empty-string value mapping before reaching Reka UI', () => {
    const select = readFileSync(new URL('../select/Select.vue', import.meta.url), 'utf8')
    const item = readFileSync(new URL('../select/SelectItem.vue', import.meta.url), 'utf8')

    expect(select).toContain('fromSelectInternalValue')
    expect(select).toContain('useForwardProps(props)')
    expect(select).toContain('@update:model-value="handleUpdateModelValue"')
    expect(item).toContain('toSelectInternalItemValue')
    expect(item).toContain('value: toSelectInternalItemValue(props.value)')
  })
})
