export type DurationUnit = 'second' | 'minute' | 'hour' | 'day'

export const durationUnitOptions: Array<{ value: DurationUnit; labelKey: string; factor: number }> = [
  { value: 'second', labelKey: 'unit.second', factor: 1 },
  { value: 'minute', labelKey: 'unit.minute', factor: 60 },
  { value: 'hour', labelKey: 'unit.hour', factor: 3600 },
  { value: 'day', labelKey: 'unit.day', factor: 86_400 },
]

export function unitInputToDurationSeconds(value: number | string, unit: DurationUnit) {
  const parsed = typeof value === 'number' ? value : Number.parseFloat(value)
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return 0
  }
  const factor = durationUnitOptions.find((option) => option.value === unit)?.factor ?? 1
  return Math.round(parsed * factor)
}

export function durationToUnitInput(seconds: number | string, defaultUnit: DurationUnit = 'hour') {
  const parsed = typeof seconds === 'number' ? seconds : Number(seconds)
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return { value: 0, unit: defaultUnit }
  }

  const unit = [...durationUnitOptions]
    .reverse()
    .find((option) => parsed >= option.factor && parsed % option.factor === 0)
    ?? durationUnitOptions[0]

  return {
    value: parsed / unit.factor,
    unit: unit.value,
  }
}
