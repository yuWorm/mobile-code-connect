<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { HardDrive, Loader2, ShieldCheck, Trash2, UserPlus } from 'lucide-vue-next'

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
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { createAccessGrantRequest } from '@/lib/control/forms'
import { formatDeviceStatus } from '@/lib/control/labels'
import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '@/lib/control/select-filter'
import type { Device, DeviceAccessGrant } from '@/lib/control/types'

const q = ref('')
const status = ref('')
const userId = ref('')
const sort = ref('device_id')
const accessQ = ref('')
const accessOpen = ref(false)
const accessLoading = ref(false)
const granting = ref(false)
const selectedDevice = ref<Device | null>(null)
const { t } = useI18n()
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
const grants = ref<DeviceAccessGrant[]>([])
const accessTotal = ref(0)
const grantForm = reactive({ user_id: '' })
const query = computed(() => ({
  q: q.value.trim(),
  status: status.value,
  user_id: userId.value.trim(),
  limit: 100,
  sort: sort.value,
}))
const hasDeviceFilters = computed(() =>
  q.value.trim() !== '' ||
  status.value !== '' ||
  userId.value.trim() !== '' ||
  sort.value !== 'device_id',
)
const hasAccessFilters = computed(() =>
  accessQ.value.trim() !== '',
)
const devices = useAsyncData(() => controlApi.devices(query.value))
watch([q, status, userId, sort], () => devices.refresh())
watch(accessQ, () => refreshAccess())

function resetDeviceFilters() {
  q.value = ''
  status.value = ''
  userId.value = ''
  sort.value = 'device_id'
}

function resetAccessFilters() {
  accessQ.value = ''
}

function resetGrantForm() {
  grantForm.user_id = ''
}

function resetAccessDialogState() {
  selectedDevice.value = null
  grants.value = []
  accessTotal.value = 0
  resetAccessFilters()
  resetGrantForm()
}

function handleAccessOpenChange(nextOpen: boolean) {
  if (granting.value && !nextOpen) {
    return
  }
  accessOpen.value = nextOpen
  if (!nextOpen) {
    resetAccessDialogState()
  }
}

async function removeDevice(deviceId: string) {
  await runBusyAction(`remove:${deviceId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.removeDevice(deviceId)
        await devices.refresh()
      },
      {
        success: t('device.toast.removed'),
        error: t('device.toast.removeFailed'),
      },
    )
  })
}

async function openAccess(device: Device) {
  selectedDevice.value = device
  accessOpen.value = true
  accessQ.value = ''
  grants.value = []
  accessTotal.value = 0
  resetGrantForm()
  await refreshAccess()
}

async function refreshAccess() {
  if (!selectedDevice.value) {
    return
  }
  accessLoading.value = true
  try {
    const page = await controlApi.deviceAccess(selectedDevice.value.device_id, {
      q: accessQ.value.trim(),
      limit: 100,
      sort: 'user_id',
    })
    grants.value = page.items
    accessTotal.value = page.total
  } finally {
    accessLoading.value = false
  }
}

async function grantAccess() {
  if (!selectedDevice.value || granting.value || !grantForm.user_id.trim()) {
    return
  }
  granting.value = true
  try {
    await runWithToast(
      async () => {
        await controlApi.grantDeviceAccess(
          selectedDevice.value!.device_id,
          createAccessGrantRequest(grantForm),
        )
        resetGrantForm()
        await refreshAccess()
      },
      {
        success: t('device.toast.granted'),
        error: t('device.toast.grantFailed'),
      },
    )
  } finally {
    granting.value = false
  }
}

async function revokeAccess(userId: string) {
  if (!selectedDevice.value) {
    return
  }
  await runBusyAction(`revoke:${userId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.revokeDeviceAccess(selectedDevice.value!.device_id, userId)
        await refreshAccess()
      },
      {
        success: t('device.toast.revoked'),
        error: t('device.toast.revokeFailed'),
      },
    )
  })
}
</script>

<template>
  <main class="page-container">
    <PageSection :title="t('route.admin.devices.title')" :description="t('route.admin.devices.description')">
      <div class="grid gap-4">
        <SearchToolbar v-model="q" :placeholder="t('device.searchPlaceholder')" :loading="devices.loading.value" @refresh="devices.refresh" />
        <div class="grid gap-3 lg:grid-cols-[180px_180px_minmax(0,1fr)_auto_auto] lg:items-center">
          <Select :model-value="selectFilterValue(status)" @update:model-value="status = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('device.filterStatus')"><SelectValue :placeholder="t('common.allStatus')" /></SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('common.allStatus') }}</SelectItem>
              <SelectItem value="online">{{ formatDeviceStatus('online', t) }}</SelectItem>
              <SelectItem value="offline">{{ formatDeviceStatus('offline', t) }}</SelectItem>
            </SelectContent>
          </Select>
          <Select v-model="sort">
            <SelectTrigger :aria-label="t('device.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="device_id">{{ t('common.deviceId') }}</SelectItem>
              <SelectItem value="name">{{ t('common.deviceName') }}</SelectItem>
              <SelectItem value="status">{{ t('common.status') }}</SelectItem>
            </SelectContent>
          </Select>
          <Input v-model="userId" :placeholder="t('common.exactUserId')" :aria-label="t('common.exactUserId')" />
          <Button variant="outline" :disabled="!hasDeviceFilters" @click="resetDeviceFilters">
            {{ t('common.reset') }}
          </Button>
          <p class="text-sm text-muted-foreground lg:text-right">
            {{ t('device.total', { total: devices.data.value?.total ?? 0 }) }}
          </p>
        </div>
        <ResponsiveTable
          :items="devices.data.value?.items ?? []"
          :loading="devices.loading.value"
          :error="devices.error.value"
          :empty-title="t('device.empty')"
          @retry="devices.refresh"
        >
          <template #head>
            <TableRow>
              <TableHead>{{ t('common.device') }}</TableHead>
              <TableHead>{{ t('common.user') }}</TableHead>
              <TableHead>{{ t('device.agentVersion') }}</TableHead>
              <TableHead>{{ t('common.status') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="device in devices.data.value?.items ?? []" :key="device.device_id">
              <TableCell>
                <div class="font-medium">{{ device.name }}</div>
                <div class="text-xs text-muted-foreground">{{ device.device_id }}</div>
              </TableCell>
              <TableCell>{{ device.user_id }}</TableCell>
              <TableCell>{{ device.agent_version || '-' }}</TableCell>
              <TableCell>
                <Badge :variant="device.status === 'online' ? 'success' : 'secondary'">{{ formatDeviceStatus(device.status, t) }}</Badge>
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button variant="outline" size="sm" @click="openAccess(device)">
                    <ShieldCheck class="size-4" />
                    {{ t('device.access') }}
                  </Button>
                  <ConfirmAction
                    :title="t('device.removeTitle')"
                    :description="t('device.removeDescription', { name: device.name })"
                    :confirm-text="t('common.remove')"
                    variant="outline"
                    :icon="Trash2"
                    :disabled="hasBusyAction"
                    :loading="isBusy('remove', device.device_id)"
                    @confirm="removeDevice(device.device_id)"
                  >
                    {{ t('common.remove') }}
                  </ConfirmAction>
                </div>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="device in devices.data.value?.items ?? []" :key="device.device_id" class="rounded-md border p-4">
              <div class="flex items-start justify-between gap-3">
                <div class="flex min-w-0 items-center gap-2">
                  <HardDrive class="size-4 text-muted-foreground" />
                  <p class="truncate font-medium">{{ device.name }}</p>
                </div>
                <Badge :variant="device.status === 'online' ? 'success' : 'secondary'">{{ formatDeviceStatus(device.status, t) }}</Badge>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('common.deviceId')" :value="device.device_id" />
                <InfoRow :label="t('common.user')" :value="device.user_id" />
                <InfoRow :label="t('device.agentVersion')" :value="device.agent_version || '-'" />
              </div>
              <div class="mt-3 grid grid-cols-2 gap-2">
                <Button variant="outline" size="sm" @click="openAccess(device)">
                  <ShieldCheck class="size-4" />
                  {{ t('device.access') }}
                </Button>
                <ConfirmAction
                  :title="t('device.removeTitle')"
                  :description="t('device.removeDescription', { name: device.name })"
                  :confirm-text="t('common.remove')"
                  variant="outline"
                  :icon="Trash2"
                  :disabled="hasBusyAction"
                  :loading="isBusy('remove', device.device_id)"
                  @confirm="removeDevice(device.device_id)"
                >
                  {{ t('common.remove') }}
                </ConfirmAction>
              </div>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>

    <Dialog :open="accessOpen" @update:open="handleAccessOpenChange">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{{ t('device.accessTitle') }}</DialogTitle>
          <DialogDescription>
            {{ selectedDevice?.name }} / {{ selectedDevice?.device_id }}
          </DialogDescription>
        </DialogHeader>

        <form class="grid gap-3" @submit.prevent="grantAccess">
          <div class="grid gap-2">
            <Label for="grant-user-id">{{ t('device.grantUserId') }}</Label>
            <Input id="grant-user-id" v-model="grantForm.user_id" required placeholder="user_xxx" />
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              :disabled="granting || !grantForm.user_id.trim()"
              @click="resetGrantForm"
            >
              {{ t('common.reset') }}
            </Button>
            <Button type="submit" :disabled="granting || !grantForm.user_id.trim()">
              <Loader2 v-if="granting" class="animate-spin" />
              <UserPlus v-else class="size-4" />
              {{ t('device.grantUser') }}
            </Button>
          </DialogFooter>
        </form>

        <div class="grid gap-2">
          <div class="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-center">
            <p class="text-sm font-medium">{{ t('device.grantedUsers') }}</p>
            <Button variant="outline" size="sm" :disabled="accessLoading" @click="refreshAccess">
              <Loader2 v-if="accessLoading" class="animate-spin" />
              {{ t('common.refresh') }}
            </Button>
          </div>
          <div class="grid gap-3 sm:grid-cols-[1fr_auto_auto] sm:items-center">
            <SearchToolbar v-model="accessQ" :placeholder="t('device.accessSearchPlaceholder')" :loading="accessLoading" @refresh="refreshAccess" />
            <Button variant="outline" :disabled="!hasAccessFilters" @click="resetAccessFilters">
              {{ t('common.reset') }}
            </Button>
            <p class="text-sm text-muted-foreground sm:text-right">{{ t('device.accessTotal', { total: accessTotal }) }}</p>
          </div>
          <div v-if="accessLoading" class="rounded-md border p-4 text-center text-sm text-muted-foreground">
            {{ t('device.accessLoading') }}
          </div>
          <div v-else-if="grants.length === 0" class="rounded-md border p-4 text-center text-sm text-muted-foreground">
            {{ t('device.accessEmpty') }}
          </div>
          <div v-else class="grid gap-2">
            <div
              v-for="grant in grants"
              :key="grant.user_id"
              class="flex items-center justify-between gap-3 rounded-md border p-3"
            >
              <div class="min-w-0">
                <p class="break-all text-sm font-medium">{{ grant.user_id }}</p>
                <p class="break-all text-xs text-muted-foreground">{{ grant.device_id }}</p>
              </div>
              <ConfirmAction
                :title="t('device.revokeTitle')"
                :description="t('device.revokeDescription', { user: grant.user_id })"
                :confirm-text="t('device.revoke')"
                variant="outline"
                :icon="Trash2"
                :disabled="hasBusyAction"
                :loading="isBusy('revoke', grant.user_id)"
                @confirm="revokeAccess(grant.user_id)"
              >
                {{ t('device.revoke') }}
              </ConfirmAction>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  </main>
</template>
