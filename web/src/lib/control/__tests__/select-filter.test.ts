import { describe, expect, test } from 'bun:test'

import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '../select-filter'

describe('select filter helpers', () => {
  test('maps empty filters to a non-empty select item value', () => {
    expect(ALL_SELECT_VALUE).not.toBe('')
    expect(selectFilterValue('')).toBe(ALL_SELECT_VALUE)
    expect(selectFilterValue('online')).toBe('online')
  })

  test('maps the all sentinel back to an empty filter value', () => {
    expect(normalizeSelectFilterValue(ALL_SELECT_VALUE)).toBe('')
    expect(normalizeSelectFilterValue('false')).toBe('false')
    expect(normalizeSelectFilterValue(123)).toBe('123')
  })
})
