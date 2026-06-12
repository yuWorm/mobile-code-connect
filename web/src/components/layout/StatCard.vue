<script setup lang="ts">
import type { Component } from 'vue'

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

defineProps<{
  label: string
  value: string | number
  description?: string
  icon: Component
}>()
</script>

<template>
  <Card>
    <CardHeader class="flex flex-row items-center justify-between space-y-0 pb-2">
      <CardTitle class="text-sm font-medium text-muted-foreground">{{ label }}</CardTitle>
      <component :is="icon" class="size-4 text-muted-foreground" />
    </CardHeader>
    <CardContent>
      <div class="text-2xl font-semibold tracking-normal">{{ value }}</div>
      <p v-if="description" class="mt-1 text-xs text-muted-foreground">{{ description }}</p>
    </CardContent>
  </Card>
</template>
