export const ALL_SELECT_VALUE = '__all__'

export function selectFilterValue(value: string | null | undefined) {
  return value || ALL_SELECT_VALUE
}

export function normalizeSelectFilterValue(value: unknown) {
  const normalized = String(value)
  return normalized === ALL_SELECT_VALUE ? '' : normalized
}
