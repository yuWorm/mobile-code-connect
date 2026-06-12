<script setup lang="ts">
import type { HTMLAttributes } from 'vue'

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { cn } from '@/lib/utils'

const props = defineProps<{
  title: string
  description?: string
  class?: HTMLAttributes['class']
}>()
</script>

<template>
  <Card :class="cn('overflow-hidden', props.class)">
    <CardHeader class="flex gap-3 sm:flex-row sm:items-start sm:justify-between">
      <div class="min-w-0">
        <CardTitle>{{ title }}</CardTitle>
        <CardDescription v-if="description" class="mt-1">{{ description }}</CardDescription>
      </div>
      <div v-if="$slots.actions" class="flex shrink-0 flex-wrap items-center gap-2">
        <slot name="actions" />
      </div>
    </CardHeader>
    <CardContent>
      <slot />
    </CardContent>
  </Card>
</template>
