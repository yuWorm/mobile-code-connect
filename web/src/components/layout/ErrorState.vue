<script setup lang="ts">
import { RefreshCw } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { useI18n } from '@/composables/useI18n'

defineProps<{ message: string }>()
defineEmits<{ retry: [] }>()
const { t } = useI18n()
</script>

<template>
  <div role="alert" aria-live="assertive" class="flex min-h-32 flex-col items-center justify-center gap-3 rounded-md border border-destructive/30 bg-destructive/5 p-5 text-center">
    <p class="max-w-lg text-sm text-destructive">{{ message }}</p>
    <Button type="button" variant="outline" size="sm" @click="$emit('retry')">
      <RefreshCw class="size-4" aria-hidden="true" />
      {{ t('common.retry') }}
    </Button>
  </div>
</template>
