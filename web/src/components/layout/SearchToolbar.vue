<script setup lang="ts">
import { RefreshCw, Search } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useI18n } from '@/composables/useI18n'

const model = defineModel<string>({ default: '' })
defineProps<{ placeholder?: string; loading?: boolean; searchLabel?: string; refreshLabel?: string }>()
defineEmits<{ refresh: [] }>()
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
    <div class="relative min-w-0 flex-1">
      <Search class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
      <Input
        v-model="model"
        :placeholder="placeholder ?? t('common.search')"
        :aria-label="searchLabel ?? placeholder ?? t('common.search')"
        class="pl-9"
      />
    </div>
    <Button type="button" variant="outline" :disabled="loading" :aria-label="refreshLabel ?? t('common.refreshList')" @click="$emit('refresh')">
      <RefreshCw :class="['size-4', loading ? 'animate-spin' : '']" aria-hidden="true" />
      {{ t('common.refresh') }}
    </Button>
  </div>
</template>
