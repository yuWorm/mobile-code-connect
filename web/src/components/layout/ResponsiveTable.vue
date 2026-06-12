<script setup lang="ts" generic="T">
import { computed } from 'vue'
import { FileQuestion } from 'lucide-vue-next'

import EmptyState from './EmptyState.vue'
import ErrorState from './ErrorState.vue'
import LoadingState from './LoadingState.vue'
import { Table, TableBody, TableHeader } from '@/components/ui/table'
import { useI18n } from '@/composables/useI18n'

const props = defineProps<{
  items: T[]
  loading?: boolean
  error?: string
  emptyTitle?: string
  emptyDescription?: string
}>()

defineEmits<{ retry: [] }>()

const hasItems = computed(() => props.items.length > 0)
const { t } = useI18n()
</script>

<template>
  <LoadingState v-if="loading" />
  <ErrorState v-else-if="error" :message="error" @retry="$emit('retry')" />
  <EmptyState
    v-else-if="!hasItems"
    :icon="FileQuestion"
    :title="emptyTitle ?? t('common.emptyData')"
    :description="emptyDescription"
  />
  <div v-else>
    <div class="hidden md:block">
      <Table>
        <TableHeader>
          <slot name="head" />
        </TableHeader>
        <TableBody>
          <slot name="rows" />
        </TableBody>
      </Table>
    </div>
    <div class="grid gap-3 md:hidden">
      <slot name="cards" />
    </div>
  </div>
</template>
