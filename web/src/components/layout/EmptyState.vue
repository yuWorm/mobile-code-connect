<script setup lang="ts">
import type { Component } from 'vue'

defineProps<{
  title: string
  description?: string
  icon?: Component
}>()
</script>

<template>
  <div role="status" aria-live="polite" class="flex min-h-40 flex-col items-center justify-center rounded-md border border-dashed p-6 text-center">
    <component :is="icon" v-if="icon" class="mb-3 size-8 text-muted-foreground" aria-hidden="true" />
    <p class="text-sm font-medium">{{ title }}</p>
    <p v-if="description" class="mt-1 max-w-sm text-sm text-muted-foreground">{{ description }}</p>
  </div>
</template>
