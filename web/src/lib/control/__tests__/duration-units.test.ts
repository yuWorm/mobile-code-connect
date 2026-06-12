import { describe, expect, test } from 'bun:test'

import { durationToUnitInput, unitInputToDurationSeconds } from '../duration-units'

describe('duration unit helpers', () => {
  test('present seconds as human editable units', () => {
    expect(durationToUnitInput(3600)).toEqual({ value: 1, unit: 'hour' })
    expect(durationToUnitInput(86_400)).toEqual({ value: 1, unit: 'day' })
  })

  test('convert human duration units back to seconds', () => {
    expect(unitInputToDurationSeconds(30, 'minute')).toBe(1800)
    expect(unitInputToDurationSeconds(1.5, 'hour')).toBe(5400)
  })
})
