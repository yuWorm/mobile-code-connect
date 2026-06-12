<script setup lang="ts">
import { TabsContent, type TabsContentProps } from 'reka-ui'
import type { HTMLAttributes } from 'vue'

import { cn } from '@/lib/utils'

const props = defineProps<TabsContentProps & { class?: HTMLAttributes['class'] }>()
</script>

<template>
  <TabsContent v-bind="props" :class="cn('mt-4 focus-visible:outline-none', props.class)">
    <slot />
  </TabsContent>
</template>
