<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Ban, Eye, RadioTower } from 'lucide-vue-next'

import ConfirmAction from '@/components/layout/ConfirmAction.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { formatEpoch } from '@/lib/control/format'
import { formatSessionStatus } from '@/lib/control/labels'
import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '@/lib/control/select-filter'
import { canCloseAdminSession } from '@/lib/control/session'
import type { AdminSessionSummary } from '@/lib/control/types'

const q = ref('')
const status = ref('')
const userId = ref('')
const deviceId = ref('')
const sort = ref('-expire_at')
const detailOpen = ref(false)
const selectedSession = ref<AdminSessionSummary | null>(null)
const { t } = useI18n()
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
const query = computed(() => ({
  q: q.value.trim(),
  status: status.value,
  user_id: userId.value.trim(),
  device_id: deviceId.value.trim(),
  limit: 100,
  sort: sort.value,
}))
const hasSessionFilters = computed(() =>
  q.value.trim() !== '' ||
  status.value !== '' ||
  userId.value.trim() !== '' ||
  deviceId.value.trim() !== '' ||
  sort.value !== '-expire_at',
)
const sessions = useAsyncData(() => controlApi.sessions(query.value))
watch([q, status, userId, deviceId, sort], () => sessions.refresh())

function statusVariant(value: string) {
  if (value === 'bound') return 'success'
  if (value === 'pending' || value === 'claimed') return 'warning'
  return 'secondary'
}

async function closeSession(sessionId: string) {
  await runBusyAction(`close:${sessionId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.closeSession(sessionId)
        await sessions.refresh()
        if (selectedSession.value?.session_id === sessionId) {
          selectedSession.value = {
            ...selectedSession.value,
            status: 'closed',
          }
        }
      },
      {
        success: t('session.toast.closed'),
        error: t('session.toast.closeFailed'),
      },
    )
  })
}

function openSessionDetail(session: AdminSessionSummary) {
  selectedSession.value = session
  detailOpen.value = true
}

function handleDetailOpenChange(nextOpen: boolean) {
  detailOpen.value = nextOpen
  if (!nextOpen) {
    selectedSession.value = null
  }
}

function resetSessionFilters() {
  q.value = ''
  status.value = ''
  userId.value = ''
  deviceId.value = ''
  sort.value = '-expire_at'
}
</script>

<template>
  <main class="page-container">
    <PageSection :title="t('route.admin.sessions.title')" :description="t('route.admin.sessions.description')">
      <div class="grid gap-4">
        <div class="grid gap-2 lg:grid-cols-[minmax(0,1fr)_180px_180px_auto]">
          <SearchToolbar v-model="q" :placeholder="t('session.searchPlaceholder')" :loading="sessions.loading.value" @refresh="sessions.refresh" />
          <Select :model-value="selectFilterValue(status)" @update:model-value="status = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('session.filterStatus')"><SelectValue :placeholder="t('common.allStatus')" /></SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('common.allStatus') }}</SelectItem>
              <SelectItem value="pending">{{ formatSessionStatus('pending', t) }}</SelectItem>
              <SelectItem value="claimed">{{ formatSessionStatus('claimed', t) }}</SelectItem>
              <SelectItem value="bound">{{ formatSessionStatus('bound', t) }}</SelectItem>
              <SelectItem value="closed">{{ formatSessionStatus('closed', t) }}</SelectItem>
              <SelectItem value="expired">{{ formatSessionStatus('expired', t) }}</SelectItem>
            </SelectContent>
          </Select>
          <Select v-model="sort">
            <SelectTrigger :aria-label="t('session.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="-expire_at">{{ t('common.expiresAt') }}</SelectItem>
              <SelectItem value="status">{{ t('common.status') }}</SelectItem>
              <SelectItem value="user_email">{{ t('session.sortUserEmail') }}</SelectItem>
              <SelectItem value="session_id">{{ t('common.sessionId') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasSessionFilters" @click="resetSessionFilters">
            {{ t('common.reset') }}
          </Button>
        </div>
        <div class="grid gap-3 md:grid-cols-[1fr_1fr_auto] md:items-center">
          <Input v-model="userId" :placeholder="t('common.exactUserId')" :aria-label="t('common.exactUserId')" />
          <Input v-model="deviceId" :placeholder="t('common.exactDeviceId')" :aria-label="t('common.exactDeviceId')" />
          <p class="text-sm text-muted-foreground md:text-right">{{ t('session.total', { total: sessions.data.value?.total ?? 0 }) }}</p>
        </div>
        <ResponsiveTable
          :items="sessions.data.value?.items ?? []"
          :loading="sessions.loading.value"
          :error="sessions.error.value"
          :empty-title="t('session.empty')"
          @retry="sessions.refresh"
        >
          <template #head>
            <TableRow>
              <TableHead>{{ t('target.session') }}</TableHead>
              <TableHead>{{ t('common.user') }}</TableHead>
              <TableHead>{{ t('session.targetService') }}</TableHead>
              <TableHead>{{ t('common.relay') }}</TableHead>
              <TableHead>{{ t('common.status') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="session in sessions.data.value?.items ?? []" :key="session.session_id">
              <TableCell>
                <div class="font-medium">{{ session.session_id }}</div>
                <div class="text-xs text-muted-foreground">{{ t('session.expiresPrefix', { time: formatEpoch(session.expire_at) }) }}</div>
              </TableCell>
              <TableCell>{{ session.user_email }}</TableCell>
              <TableCell>{{ session.device_name }} / {{ session.service_name }}</TableCell>
              <TableCell>{{ session.relay_addr }}</TableCell>
              <TableCell><Badge :variant="statusVariant(session.status)">{{ formatSessionStatus(session.status, t) }}</Badge></TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button variant="outline" size="sm" @click="openSessionDetail(session)">
                    <Eye class="size-4" />
                    {{ t('common.details') }}
                  </Button>
                  <ConfirmAction
                    :title="t('session.closeTitle')"
                    :description="t('session.closeDescription', { id: session.session_id })"
                    :confirm-text="t('common.close')"
                    variant="outline"
                    :icon="Ban"
                    :disabled="hasBusyAction || !canCloseAdminSession(session)"
                    :loading="isBusy('close', session.session_id)"
                    @confirm="closeSession(session.session_id)"
                  >
                    {{ t('common.close') }}
                  </ConfirmAction>
                </div>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="session in sessions.data.value?.items ?? []" :key="session.session_id" class="rounded-md border p-4">
              <div class="flex items-start justify-between gap-3">
                <div class="flex min-w-0 items-center gap-2">
                  <RadioTower class="size-4 text-muted-foreground" />
                  <p class="truncate font-medium">{{ session.session_id }}</p>
                </div>
                <Badge :variant="statusVariant(session.status)">{{ formatSessionStatus(session.status, t) }}</Badge>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('common.user')" :value="session.user_email" />
                <InfoRow :label="t('common.device')" :value="session.device_name" />
                <InfoRow :label="t('common.service')" :value="session.service_name" />
                <InfoRow :label="t('common.relay')" :value="session.relay_addr" />
                <InfoRow :label="t('common.expiresAt')" :value="formatEpoch(session.expire_at)" />
              </div>
              <div class="mt-3 grid grid-cols-2 gap-2">
                <Button variant="outline" size="sm" @click="openSessionDetail(session)">
                  <Eye class="size-4" />
                  {{ t('common.details') }}
                </Button>
                <ConfirmAction
                  :title="t('session.closeTitle')"
                  :description="t('session.closeDescription', { id: session.session_id })"
                  :confirm-text="t('common.close')"
                  variant="outline"
                  :icon="Ban"
                  :disabled="hasBusyAction || !canCloseAdminSession(session)"
                  :loading="isBusy('close', session.session_id)"
                  @confirm="closeSession(session.session_id)"
                >
                  {{ t('common.close') }}
                </ConfirmAction>
              </div>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>

    <Dialog :open="detailOpen" @update:open="handleDetailOpenChange">
      <DialogContent class="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{{ t('session.detailTitle') }}</DialogTitle>
          <DialogDescription>
            {{ selectedSession?.session_id }}
          </DialogDescription>
        </DialogHeader>

        <div v-if="selectedSession" class="grid gap-4">
          <div class="flex flex-wrap items-center gap-2">
            <Badge :variant="statusVariant(selectedSession.status)">{{ formatSessionStatus(selectedSession.status, t) }}</Badge>
            <Badge class="min-w-0 max-w-full truncate" variant="outline">{{ selectedSession.client_id }}</Badge>
          </div>

          <div class="grid gap-3 sm:grid-cols-2">
            <div class="rounded-md border p-3">
              <p class="text-sm font-medium">{{ t('session.userPanel') }}</p>
              <div class="mt-2">
                <InfoRow :label="t('common.email')" :value="selectedSession.user_email" />
                <InfoRow :label="t('common.userId')" :value="selectedSession.user_id" />
              </div>
            </div>
            <div class="rounded-md border p-3">
              <p class="text-sm font-medium">{{ t('session.targetPanel') }}</p>
              <div class="mt-2">
                <InfoRow :label="t('common.device')" :value="selectedSession.device_name" />
                <InfoRow :label="t('common.deviceId')" :value="selectedSession.device_id" />
                <InfoRow :label="t('common.service')" :value="selectedSession.service_name" />
                <InfoRow :label="t('common.serviceId')" :value="selectedSession.service_id" />
              </div>
            </div>
          </div>

          <div class="rounded-md border p-3">
            <p class="text-sm font-medium">{{ t('session.connectionInfo') }}</p>
            <div class="mt-2">
              <InfoRow :label="t('common.relay')" :value="selectedSession.relay_addr" />
              <InfoRow :label="t('common.punch')" :value="selectedSession.punch_addr" />
              <InfoRow :label="t('common.expiresAt')" :value="formatEpoch(selectedSession.expire_at)" />
            </div>
          </div>
        </div>

        <DialogFooter v-if="selectedSession">
          <ConfirmAction
            :title="t('session.closeTitle')"
            :description="t('session.closeDescription', { id: selectedSession.session_id })"
            :confirm-text="t('common.close')"
            variant="outline"
            size="default"
            :icon="Ban"
            :disabled="hasBusyAction || !canCloseAdminSession(selectedSession)"
            :loading="isBusy('close', selectedSession.session_id)"
            @confirm="closeSession(selectedSession.session_id)"
          >
            {{ t('session.closeTitle') }}
          </ConfirmAction>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </main>
</template>
