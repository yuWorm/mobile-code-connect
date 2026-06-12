<script setup lang="ts">
import {
  DialogClose,
  DialogContent,
  DialogOverlay,
  DialogPortal,
  type DialogContentEmits,
  type DialogContentProps,
  useForwardPropsEmits,
} from 'reka-ui'
import { X } from 'lucide-vue-next'
import type { HTMLAttributes } from 'vue'

import { useI18n } from '@/composables/useI18n'
import { cn } from '@/lib/utils'

const props = defineProps<DialogContentProps & { class?: HTMLAttributes['class'] }>()
const emits = defineEmits<DialogContentEmits>()
const forwarded = useForwardPropsEmits(props, emits)
const { t } = useI18n()
</script>

<template>
  <DialogPortal>
    <DialogOverlay class="fixed inset-0 z-50 bg-black/55 backdrop-blur-sm data-[state=open]:animate-in" />
    <DialogContent
      v-bind="forwarded"
      :class="
        cn(
          'fixed left-1/2 top-1/2 z-50 grid max-h-[calc(100dvh-2rem)] w-[calc(100%-2rem)] max-w-lg -translate-x-1/2 -translate-y-1/2 gap-4 overflow-y-auto rounded-lg border bg-popover p-5 text-popover-foreground shadow-xl sm:w-full',
          props.class,
        )
      "
    >
      <slot />
      <DialogClose
        class="focus-ring absolute right-3 top-3 rounded-md p-1 text-muted-foreground transition-colors hover:text-foreground"
      >
        <X class="size-4" />
        <span class="sr-only">{{ t('common.close') }}</span>
      </DialogClose>
    </DialogContent>
  </DialogPortal>
</template>
