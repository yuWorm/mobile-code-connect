import { describe, expect, test } from 'bun:test'

import { bytesToUnitInput, unitInputToBytes } from '../byte-units'

describe('byte unit helpers', () => {
  test('present byte counts as human editable units', () => {
    expect(bytesToUnitInput(104_857_600)).toEqual({ value: 100, unit: 'MB' })
    expect(bytesToUnitInput(1_073_741_824)).toEqual({ value: 1, unit: 'GB' })
  })

  test('convert human units back to backend byte values', () => {
    expect(unitInputToBytes(100, 'MB')).toBe(104_857_600)
    expect(unitInputToBytes(1.5, 'GB')).toBe(1_610_612_736)
  })
})
