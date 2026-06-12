<script setup lang="ts">
import { TabsTrigger, type TabsTriggerProps } from 'reka-ui'
import type { HTMLAttributes } from 'vue'

import { cn } from '@/lib/utils'

const props = defineProps<TabsTriggerProps & { class?: HTMLAttributes['class'] }>()
</script>

<template>
  <TabsTrigger
    v-bind="props"
    :class="
      cn(
        'focus-ring inline-flex h-7 items-center justify-center whitespace-nowrap rounded-sm px-3 text-sm font-medium transition-all data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm',
        props.class,
      )
    "
  >
    <slot />
  </TabsTrigger>
</template>
