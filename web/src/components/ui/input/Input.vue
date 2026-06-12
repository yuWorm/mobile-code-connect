<script setup lang="ts">
import type { HTMLAttributes } from 'vue'

import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()

const [model, modelModifiers] = defineModel<string | number, 'number'>({
  set(value) {
    if (!modelModifiers.number || typeof value !== 'string') {
      return value
    }

    const parsed = Number.parseFloat(value)
    return Number.isNaN(parsed) ? value : parsed
  },
})
</script>

<template>
  <input
    v-model="model"
    :class="
      cn(
        'focus-ring flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50',
        props.class,
      )
    "
  />
</template>
