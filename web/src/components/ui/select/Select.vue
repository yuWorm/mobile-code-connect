<script setup lang="ts">
import { SelectRoot, type SelectRootEmits, type SelectRootProps, useForwardProps } from 'reka-ui'

import { fromSelectInternalValue } from './empty-value'

const props = defineProps<SelectRootProps<string>>()
const emits = defineEmits<SelectRootEmits<string>>()
const forwarded = useForwardProps(props)

function handleUpdateModelValue(value: string) {
  emits('update:modelValue', fromSelectInternalValue(value))
}

function handleUpdateOpen(value: boolean) {
  emits('update:open', value)
}
</script>

<template>
  <SelectRoot v-bind="forwarded" @update:model-value="handleUpdateModelValue" @update:open="handleUpdateOpen">
    <slot />
  </SelectRoot>
</template>
