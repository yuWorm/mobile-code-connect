import { describe, expect, test } from 'bun:test'

import { formatBytes, formatEpoch, formatPercent } from '../format'

describe('formatBytes', () => {
  test('formats byte values with compact units', () => {
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatBytes(5 * 1024 * 1024)).toBe('5 MB')
  })
})

describe('formatEpoch', () => {
  test('formats zero-like epoch values as an empty marker', () => {
    expect(formatEpoch(0)).toBe('-')
  })
})

describe('formatPercent', () => {
  test('handles empty totals without dividing by zero', () => {
    expect(formatPercent(10, 0)).toBe('0%')
    expect(formatPercent(25, 100)).toBe('25%')
  })
})
