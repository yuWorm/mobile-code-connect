<script setup lang="ts">
import { Check } from 'lucide-vue-next'
import { SelectItem, SelectItemIndicator, SelectItemText, type SelectItemProps } from 'reka-ui'
import { computed } from 'vue'
import type { HTMLAttributes } from 'vue'

import { cn } from '@/lib/utils'
import { toSelectInternalItemValue } from './empty-value'

const props = defineProps<SelectItemProps<string> & { class?: HTMLAttributes['class'] }>()
const forwarded = computed(() => ({
  ...props,
  value: toSelectInternalItemValue(props.value),
}))
</script>

<template>
  <SelectItem
    v-bind="forwarded"
    :class="
      cn(
        'focus:bg-accent focus:text-accent-foreground relative flex cursor-default select-none items-center rounded-sm py-1.5 pl-8 pr-2 text-sm outline-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
        props.class,
      )
    "
  >
    <span class="absolute left-2 flex size-3.5 items-center justify-center">
      <SelectItemIndicator>
        <Check class="size-4" />
      </SelectItemIndicator>
    </span>
    <SelectItemText>
      <slot />
    </SelectItemText>
  </SelectItem>
</template>
