<script setup lang="ts">
import { Loader2 } from 'lucide-vue-next'

import { useI18n } from '@/composables/useI18n'

defineProps<{ label?: string }>()
const { t } = useI18n()
</script>

<template>
  <div role="status" aria-live="polite" class="flex min-h-40 items-center justify-center gap-2 text-sm text-muted-foreground">
    <Loader2 class="size-4 animate-spin" aria-hidden="true" />
    {{ label ?? t('common.loading') }}
  </div>
</template>
