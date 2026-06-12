<script setup lang="ts">
defineProps<{ label: string; value?: string | number | null }>()
</script>

<template>
  <div class="flex items-start justify-between gap-3 border-b py-2 last:border-b-0">
    <span class="shrink-0 text-xs text-muted-foreground">{{ label }}</span>
    <span class="min-w-0 break-all text-right text-sm font-medium">{{ value ?? '-' }}</span>
  </div>
</template>
