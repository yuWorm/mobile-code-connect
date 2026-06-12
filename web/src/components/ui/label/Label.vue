<script setup lang="ts">
import { Label } from 'reka-ui'
import type { HTMLAttributes } from 'vue'

import { cn } from '@/lib/utils'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <Label :class="cn('text-sm font-medium leading-none', props.class)">
    <slot />
  </Label>
</template>
