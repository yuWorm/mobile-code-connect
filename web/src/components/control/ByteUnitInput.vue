<script setup lang="ts">
import { ref, watch } from 'vue'

import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { useI18n } from '@/composables/useI18n'
import {
  byteUnitOptions,
  bytesToUnitInput,
  unitInputToBytes,
  type ByteUnit,
} from '@/lib/control/byte-units'

const props = withDefaults(defineProps<{
  id: string
  label: string
  modelValue: number | string
  defaultUnit?: ByteUnit
  rate?: boolean
  disabled?: boolean
}>(), {
  defaultUnit: 'MB',
  rate: false,
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: number]
}>()

const { t } = useI18n()
const amount = ref<number | string>(0)
const unit = ref<ByteUnit>(props.defaultUnit)
const syncingFromModel = ref(false)
const lastEmittedBytes = ref<number | null>(null)

watch(
  () => props.modelValue,
  (value) => {
    if (Number(value) === lastEmittedBytes.value) {
      lastEmittedBytes.value = null
      return
    }
    syncingFromModel.value = true
    const next = bytesToUnitInput(value, props.defaultUnit)
    amount.value = next.value
    unit.value = next.unit
    queueMicrotask(() => {
      syncingFromModel.value = false
    })
  },
  { immediate: true },
)

watch([amount, unit], () => {
  if (syncingFromModel.value) {
    return
  }
  const nextBytes = unitInputToBytes(amount.value, unit.value)
  if (nextBytes !== Number(props.modelValue)) {
    lastEmittedBytes.value = nextBytes
    emit('update:modelValue', nextBytes)
  }
})
</script>

<template>
  <div class="grid gap-2">
    <Label :for="id">{{ label }}</Label>
    <div class="grid grid-cols-[minmax(0,1fr)_112px_auto] items-center gap-2">
      <Input
        :id="id"
        v-model.number="amount"
        type="number"
        min="0"
        step="0.01"
        :disabled="disabled"
      />
      <Select v-model="unit" :disabled="disabled">
        <SelectTrigger :aria-label="t('common.unitFor', { label })">
          <SelectValue :placeholder="t('common.unit')" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in byteUnitOptions"
            :key="option.value"
            :value="option.value"
          >
            {{ option.label }}
          </SelectItem>
        </SelectContent>
      </Select>
      <span class="text-sm text-muted-foreground">{{ rate ? '/s' : '' }}</span>
    </div>
  </div>
</template>
