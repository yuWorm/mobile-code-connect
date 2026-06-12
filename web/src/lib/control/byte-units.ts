export type ByteUnit = 'KB' | 'MB' | 'GB' | 'TB'

export const byteUnitOptions: Array<{ value: ByteUnit; label: ByteUnit; factor: number }> = [
  { value: 'KB', label: 'KB', factor: 1024 },
  { value: 'MB', label: 'MB', factor: 1024 ** 2 },
  { value: 'GB', label: 'GB', factor: 1024 ** 3 },
  { value: 'TB', label: 'TB', factor: 1024 ** 4 },
]

export function unitInputToBytes(value: number | string, unit: ByteUnit) {
  const parsed = typeof value === 'number' ? value : Number.parseFloat(value)
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return 0
  }
  const factor = byteUnitOptions.find((option) => option.value === unit)?.factor ?? 1
  return Math.round(parsed * factor)
}

export function bytesToUnitInput(bytes: number | string, defaultUnit: ByteUnit = 'MB') {
  const parsed = typeof bytes === 'number' ? bytes : Number(bytes)
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return { value: 0, unit: defaultUnit }
  }

  const unit = [...byteUnitOptions]
    .reverse()
    .find((option) => parsed >= option.factor)
    ?? byteUnitOptions[0]

  return {
    value: parsed / unit.factor,
    unit: unit.value,
  }
}
