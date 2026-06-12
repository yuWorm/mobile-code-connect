<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Component } from 'vue'
import { Loader2 } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { useI18n } from '@/composables/useI18n'

type MaybePromise<T> = T | Promise<T>

const props = withDefaults(
  defineProps<{
    title: string
    description: string
    confirmText?: string
    cancelText?: string
    variant?: 'default' | 'secondary' | 'destructive' | 'outline' | 'ghost' | 'link'
    size?: 'default' | 'sm' | 'lg' | 'icon'
    icon?: Component
    disabled?: boolean
    loading?: boolean
    action?: () => MaybePromise<void>
    class?: string
  }>(),
  {
    variant: 'destructive',
    size: 'sm',
    disabled: false,
    loading: false,
  },
)

const emit = defineEmits<{ confirm: [] }>()
const open = ref(false)
const internalLoading = ref(false)
const isLoading = computed(() => props.loading || internalLoading.value)
const { t } = useI18n()

async function confirm() {
  if (isLoading.value) {
    return
  }

  if (!props.action) {
    open.value = false
    emit('confirm')
    return
  }

  internalLoading.value = true
  try {
    await props.action()
    open.value = false
  } catch {
    // Keep the dialog open so the user can retry after the caller shows feedback.
  } finally {
    internalLoading.value = false
  }
}

function cancel() {
  if (isLoading.value) {
    return
  }
  open.value = false
}

function openChange(nextOpen: boolean) {
  if (isLoading.value && !nextOpen) {
    return
  }
  open.value = nextOpen
}
</script>

<template>
  <Dialog :open="open" @update:open="openChange">
    <DialogTrigger as-child>
      <Button type="button" :class="props.class" :disabled="disabled || isLoading" :size="size" :variant="variant">
        <component :is="icon" v-if="icon" class="size-4" aria-hidden="true" />
        <slot />
      </Button>
    </DialogTrigger>
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button type="button" variant="outline" :disabled="isLoading" @click="cancel">{{ cancelText ?? t('common.cancel') }}</Button>
        <Button type="button" variant="destructive" :disabled="isLoading" @click="confirm">
          <Loader2 v-if="isLoading" class="animate-spin" aria-hidden="true" />
          {{ confirmText ?? t('common.confirm') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
