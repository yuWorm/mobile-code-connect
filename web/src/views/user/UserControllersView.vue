<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Loader2, Plus, ShieldCheck, Trash2 } from 'lucide-vue-next'

import ConfirmAction from '@/components/layout/ConfirmAction.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
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
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'

const q = ref('')
const sort = ref('client_id')
const open = ref(false)
const saving = ref(false)
const form = reactive({ client_id: '', name: '' })
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
const { t } = useI18n()
const controllersQuery = computed(() => ({
  q: q.value.trim(),
  limit: 100,
  sort: sort.value,
}))
const hasControllerFilters = computed(() =>
  q.value.trim() !== '' ||
  sort.value !== 'client_id',
)
const hasControllerForm = computed(() =>
  form.client_id.trim() !== '' &&
  form.name.trim() !== '',
)
const controllers = useAsyncData(() => controlApi.controllers(controllersQuery.value))
watch([q, sort], () => controllers.refresh())

function resetControllerFilters() {
  q.value = ''
  sort.value = 'client_id'
}

function resetControllerForm() {
  form.client_id = ''
  form.name = ''
}

function handleControllerOpenChange(nextOpen: boolean) {
  if (saving.value && !nextOpen) {
    return
  }
  open.value = nextOpen
  if (!nextOpen) {
    resetControllerForm()
  }
}

async function createController() {
  if (saving.value || !hasControllerForm.value) {
    return
  }
  saving.value = true
  try {
    await runWithToast(
      async () => {
        await controlApi.registerController(form)
        resetControllerForm()
        open.value = false
        await controllers.refresh()
      },
      {
        success: t('controller.toast.created'),
        error: t('controller.toast.createFailed'),
      },
    )
  } finally {
    saving.value = false
  }
}

async function removeController(clientId: string) {
  await runBusyAction(`remove:${clientId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.removeController(clientId)
        await controllers.refresh()
      },
      {
        success: t('controller.toast.removed'),
        error: t('controller.toast.removeFailed'),
      },
    )
  })
}
</script>

<template>
  <main class="page-container">
    <PageSection :title="t('route.center.controllers.title')" :description="t('route.center.controllers.description')">
      <template #actions>
        <Dialog :open="open" @update:open="handleControllerOpenChange">
          <DialogTrigger as-child>
            <Button @click="resetControllerForm"><Plus class="size-4" />{{ t('controller.register') }}</Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{{ t('controller.register') }}</DialogTitle>
              <DialogDescription>{{ t('controller.registerDescription') }}</DialogDescription>
            </DialogHeader>
            <form class="grid gap-4" @submit.prevent="createController">
              <div class="grid gap-2">
                <Label for="controller-client-id">{{ t('common.clientId') }}</Label>
                <Input id="controller-client-id" v-model="form.client_id" required />
              </div>
              <div class="grid gap-2">
                <Label for="controller-name">{{ t('common.name') }}</Label>
                <Input id="controller-name" v-model="form.name" required />
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" :disabled="saving" @click="resetControllerForm">
                  {{ t('common.reset') }}
                </Button>
                <Button type="submit" :disabled="saving || !hasControllerForm">
                  <Loader2 v-if="saving" class="animate-spin" />
                  {{ t('common.register') }}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </template>
      <div class="grid gap-4">
        <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_220px_auto]">
          <SearchToolbar v-model="q" :placeholder="t('controller.searchPlaceholder')" :loading="controllers.loading.value" @refresh="controllers.refresh" />
          <Select v-model="sort">
            <SelectTrigger :aria-label="t('controller.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="client_id">{{ t('common.clientId') }}</SelectItem>
              <SelectItem value="-client_id">{{ t('controller.sortClientDesc') }}</SelectItem>
              <SelectItem value="name">{{ t('controller.sortName') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasControllerFilters" @click="resetControllerFilters">
            {{ t('common.reset') }}
          </Button>
        </div>
        <p class="text-sm text-muted-foreground sm:text-right">
          {{ t('controller.total', { total: controllers.data.value?.total ?? 0 }) }}
        </p>
        <ResponsiveTable :items="controllers.data.value?.items ?? []" :loading="controllers.loading.value" :error="controllers.error.value" :empty-title="t('controller.empty')" @retry="controllers.refresh">
          <template #head>
            <TableRow>
              <TableHead>{{ t('controller.table.controller') }}</TableHead>
              <TableHead>{{ t('common.user') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="controller in controllers.data.value?.items ?? []" :key="controller.client_id">
              <TableCell><div class="font-medium">{{ controller.name }}</div><div class="text-xs text-muted-foreground">{{ controller.client_id }}</div></TableCell>
              <TableCell>{{ controller.user_id }}</TableCell>
              <TableCell class="text-right">
                <ConfirmAction
                  :title="t('controller.removeTitle')"
                  :description="t('controller.removeDescription', { name: controller.name })"
                  :confirm-text="t('common.remove')"
                  variant="outline"
                  :icon="Trash2"
                  :disabled="hasBusyAction"
                  :loading="isBusy('remove', controller.client_id)"
                  @confirm="removeController(controller.client_id)"
                >
                  {{ t('common.remove') }}
                </ConfirmAction>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="controller in controllers.data.value?.items ?? []" :key="controller.client_id" class="rounded-md border p-4">
              <div class="flex items-center gap-2"><ShieldCheck class="size-4 text-muted-foreground" /><p class="font-medium">{{ controller.name }}</p></div>
              <div class="mt-3">
                <InfoRow :label="t('common.clientId')" :value="controller.client_id" />
                <InfoRow :label="t('common.user')" :value="controller.user_id" />
              </div>
              <ConfirmAction
                class="mt-3 w-full"
                :title="t('controller.removeTitle')"
                :description="t('controller.removeDescription', { name: controller.name })"
                :confirm-text="t('common.remove')"
                variant="outline"
                :icon="Trash2"
                :disabled="hasBusyAction"
                :loading="isBusy('remove', controller.client_id)"
                @confirm="removeController(controller.client_id)"
              >
                {{ t('common.remove') }}
              </ConfirmAction>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>
  </main>
</template>
