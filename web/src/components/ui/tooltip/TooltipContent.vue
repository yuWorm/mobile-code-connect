<script setup lang="ts">
import {
  TooltipContent,
  TooltipPortal,
  type TooltipContentEmits,
  type TooltipContentProps,
  useForwardPropsEmits,
} from 'reka-ui'
import type { HTMLAttributes } from 'vue'

import { cn } from '@/lib/utils'

const props = withDefaults(
  defineProps<TooltipContentProps & { class?: HTMLAttributes['class'] }>(),
  { sideOffset: 4 },
)
const emits = defineEmits<TooltipContentEmits>()
const forwarded = useForwardPropsEmits(props, emits)
</script>

<template>
  <TooltipPortal>
    <TooltipContent
      v-bind="forwarded"
      :class="cn('z-50 rounded-md bg-foreground px-2 py-1 text-xs text-background shadow-md', props.class)"
    >
      <slot />
    </TooltipContent>
  </TooltipPortal>
</template>
