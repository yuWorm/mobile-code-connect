export const EMPTY_SELECT_ITEM_VALUE = '__control_empty_select_item__'

export function toSelectInternalItemValue(value: string) {
  return value === '' ? EMPTY_SELECT_ITEM_VALUE : value
}

export function fromSelectInternalValue(value: string) {
  return value === EMPTY_SELECT_ITEM_VALUE ? '' : value
}
