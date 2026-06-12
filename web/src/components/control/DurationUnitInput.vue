<script setup lang="ts">
import { ref, watch } from 'vue'

import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { useI18n } from '@/composables/useI18n'
import {
  durationToUnitInput,
  durationUnitOptions,
  unitInputToDurationSeconds,
  type DurationUnit,
} from '@/lib/control/duration-units'

const props = withDefaults(defineProps<{
  id: string
  label: string
  modelValue: number | string
  defaultUnit?: DurationUnit
  disabled?: boolean
}>(), {
  defaultUnit: 'hour',
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: number]
}>()

const { t } = useI18n()
const amount = ref<number | string>(0)
const unit = ref<DurationUnit>(props.defaultUnit)
const syncingFromModel = ref(false)
const lastEmittedSeconds = ref<number | null>(null)

watch(
  () => props.modelValue,
  (value) => {
    if (Number(value) === lastEmittedSeconds.value) {
      lastEmittedSeconds.value = null
      return
    }
    syncingFromModel.value = true
    const next = durationToUnitInput(value, props.defaultUnit)
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
  const nextSeconds = unitInputToDurationSeconds(amount.value, unit.value)
  if (nextSeconds !== Number(props.modelValue)) {
    lastEmittedSeconds.value = nextSeconds
    emit('update:modelValue', nextSeconds)
  }
})
</script>

<template>
  <div class="grid gap-2">
    <Label :for="id">{{ label }}</Label>
    <div class="grid grid-cols-[minmax(0,1fr)_112px] items-center gap-2">
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
            v-for="option in durationUnitOptions"
            :key="option.value"
            :value="option.value"
          >
            {{ t(option.labelKey) }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
  </div>
</template>
