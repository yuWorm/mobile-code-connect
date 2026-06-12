<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { Check, Copy, HardDrive, KeyRound, ListTree, Loader2, Plus, X } from 'lucide-vue-next'

import EmptyState from '@/components/layout/EmptyState.vue'
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
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { copyToClipboard } from '@/lib/control/clipboard'
import { createSessionRequest } from '@/lib/control/forms'
import { formatEpoch } from '@/lib/control/format'
import { formatDeviceStatus } from '@/lib/control/labels'
import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '@/lib/control/select-filter'
import { buildCreatedSessionClipboardText } from '@/lib/control/session'
import type { CreateSessionResponse, Device, Service } from '@/lib/control/types'

const devices = useAsyncData(() => controlApi.mobileDevices())
const controllers = useAsyncData(() => controlApi.controllers({ limit: 100, sort: 'client_id' }))
const { t } = useI18n()
const q = ref('')
const status = ref('')
const sort = ref('name')
const serviceQ = ref('')
const services = ref<Record<string, Service[]>>({})
const serviceLoading = ref<Record<string, boolean>>({})
const sessionOpen = ref(false)
const creatingSession = ref(false)
const selectedDevice = ref<Device | null>(null)
const selectedService = ref<Service | null>(null)
const createdSession = ref<CreateSessionResponse | null>(null)
const copiedSessionField = ref<'bundle' | 'access' | 'relay' | null>(null)
const sessionForm = reactive({
  client_id: '',
  device_id: '',
  service_id: '',
})

const availableControllers = computed(() => controllers.data.value?.items ?? [])
const serviceRefreshing = computed(() =>
  Object.values(serviceLoading.value).some((loading) => loading),
)
const hasDeviceFilters = computed(() =>
  q.value.trim() !== '' ||
  status.value !== '' ||
  sort.value !== 'name',
)
const hasServiceFilters = computed(() =>
  serviceQ.value.trim() !== '',
)
const filteredDevices = computed(() => {
  const keyword = q.value.trim().toLowerCase()

  const items = (devices.data.value ?? []).filter((device) => {
    const matchesStatus = status.value === '' || device.status === status.value
    const matchesKeyword =
      keyword === '' ||
      [
        device.name,
        device.device_id,
        device.user_id,
        device.status,
        device.agent_version,
      ].some((value) => value.toLowerCase().includes(keyword))

    return matchesStatus && matchesKeyword
  })

  const sorted = [...items].sort((left: Device, right: Device) => {
    if (sort.value === 'status') {
      return left.status.localeCompare(right.status) || left.name.localeCompare(right.name)
    }
    if (sort.value === 'agent_version') {
      return (left.agent_version || '').localeCompare(right.agent_version || '') || left.name.localeCompare(right.name)
    }
    if (sort.value === 'device_id') {
      return left.device_id.localeCompare(right.device_id)
    }
    return left.name.localeCompare(right.name)
  })

  return sorted
})
const sessionClipboardText = computed(() => {
  if (!createdSession.value) {
    return ''
  }

  return buildCreatedSessionClipboardText(createdSession.value, {
    clientId: sessionForm.client_id,
    deviceId: sessionForm.device_id,
    deviceName: selectedDevice.value?.name,
    serviceId: sessionForm.service_id,
    serviceName: selectedService.value?.name,
  })
})

function filteredServices(deviceId: string) {
  const keyword = serviceQ.value.trim().toLowerCase()
  const items = services.value[deviceId] ?? []
  if (!keyword) {
    return items
  }

  return items.filter((service) =>
    [
      service.name,
      service.service_id,
      service.protocol,
      service.target_host,
      String(service.target_port),
    ].some((value) => value.toLowerCase().includes(keyword)),
  )
}

function resetDeviceFilters() {
  q.value = ''
  status.value = ''
  sort.value = 'name'
}

function resetServiceFilters() {
  serviceQ.value = ''
}

function dismissCreatedSession() {
  createdSession.value = null
  copiedSessionField.value = null
}

function resetSessionForm() {
  sessionForm.client_id = availableControllers.value[0]?.client_id ?? ''
  createdSession.value = null
  copiedSessionField.value = null
}

function resetSessionDialogState() {
  selectedDevice.value = null
  selectedService.value = null
  sessionForm.client_id = ''
  sessionForm.device_id = ''
  sessionForm.service_id = ''
  createdSession.value = null
  copiedSessionField.value = null
}

function handleSessionOpenChange(nextOpen: boolean) {
  if (creatingSession.value && !nextOpen) {
    return
  }
  sessionOpen.value = nextOpen
  if (!nextOpen) {
    resetSessionDialogState()
  }
}

async function loadServices(deviceId: string) {
  if (serviceLoading.value[deviceId]) {
    return
  }
  serviceLoading.value[deviceId] = true
  try {
    await runWithToast(
      async () => {
        const items = await controlApi.deviceServices(deviceId)
        services.value[deviceId] = items
        return items
      },
      {
        success: (items) => (items.length > 0 ? t('device.toast.servicesLoaded', { total: items.length }) : t('device.toast.noServices')),
        error: t('device.toast.loadServicesFailed'),
      },
    )
  } finally {
    serviceLoading.value[deviceId] = false
  }
}

async function refreshLoadedServices() {
  const deviceIds = Object.keys(services.value)
  if (deviceIds.length === 0) {
    return
  }

  await Promise.all(
    deviceIds.map(async (deviceId) => {
      serviceLoading.value[deviceId] = true
      try {
        services.value[deviceId] = await controlApi.deviceServices(deviceId)
      } finally {
        serviceLoading.value[deviceId] = false
      }
    }),
  )
}

async function openSessionDialog(device: Device, service: Service) {
  selectedDevice.value = device
  selectedService.value = service
  sessionForm.device_id = device.device_id
  sessionForm.service_id = service.service_id
  resetSessionForm()
  sessionOpen.value = true
}

async function createSession() {
  if (creatingSession.value || !sessionForm.client_id) {
    return
  }
  creatingSession.value = true
  createdSession.value = null
  copiedSessionField.value = null
  try {
    await runWithToast(
      async () => {
        const session = await controlApi.createSession(createSessionRequest(sessionForm))
        createdSession.value = session
        return session
      },
      {
        success: t('device.toast.sessionCreated'),
        error: t('device.toast.sessionFailed'),
      },
    )
  } finally {
    creatingSession.value = false
  }
}

async function copySessionValue(value: string, field: 'bundle' | 'access' | 'relay') {
  await copyToClipboard(value)
  copiedSessionField.value = field
  window.setTimeout(() => {
    if (copiedSessionField.value === field) {
      copiedSessionField.value = null
    }
  }, 1600)
}
</script>

<template>
  <main class="page-container">
    <PageSection :title="t('route.center.devices.title')" :description="t('route.center.devices.description')">
      <div class="grid gap-4">
        <SearchToolbar v-model="q" :placeholder="t('device.searchPlaceholder')" :loading="devices.loading.value" @refresh="devices.refresh" />
        <div class="grid gap-3 sm:grid-cols-[220px_220px_auto_1fr] sm:items-center">
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
              <SelectItem value="name">{{ t('common.deviceName') }}</SelectItem>
              <SelectItem value="status">{{ t('common.status') }}</SelectItem>
              <SelectItem value="agent_version">{{ t('device.agentVersion') }}</SelectItem>
              <SelectItem value="device_id">{{ t('common.deviceId') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasDeviceFilters" @click="resetDeviceFilters">
            {{ t('common.reset') }}
          </Button>
          <p class="text-sm text-muted-foreground sm:text-right">
            {{ t('device.accessibleTotal', { shown: filteredDevices.length, total: devices.data.value?.length ?? 0 }) }}
          </p>
        </div>
        <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]">
          <SearchToolbar v-model="serviceQ"
            :placeholder="t('device.serviceSearchPlaceholder')"
            :loading="serviceRefreshing"
            @refresh="refreshLoadedServices"
          />
          <Button variant="outline" :disabled="!hasServiceFilters" @click="resetServiceFilters">
            {{ t('common.reset') }}
          </Button>
        </div>
        <ResponsiveTable
          :items="filteredDevices"
          :loading="devices.loading.value"
          :error="devices.error.value"
          :empty-title="t('device.emptyAccessible')"
          @retry="devices.refresh"
        >
        <template #head>
          <TableRow>
            <TableHead>{{ t('common.device') }}</TableHead>
            <TableHead>{{ t('device.agentVersion') }}</TableHead>
            <TableHead>{{ t('common.status') }}</TableHead>
            <TableHead>{{ t('common.service') }}</TableHead>
            <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
          </TableRow>
        </template>
        <template #rows>
          <TableRow v-for="device in filteredDevices" :key="device.device_id">
            <TableCell>
              <div class="font-medium">{{ device.name }}</div>
              <div class="text-xs text-muted-foreground">{{ device.device_id }}</div>
            </TableCell>
            <TableCell>{{ device.agent_version || '-' }}</TableCell>
            <TableCell><Badge :variant="device.status === 'online' ? 'success' : 'secondary'">{{ formatDeviceStatus(device.status, t) }}</Badge></TableCell>
            <TableCell>
              <div v-if="services[device.device_id]" class="flex flex-wrap gap-1">
                <Button
                  v-for="service in filteredServices(device.device_id)"
                  :key="service.service_id"
                  class="max-w-full min-w-0"
                  variant="outline"
                  size="sm"
                  @click="openSessionDialog(device, service)"
                >
                  <Plus class="size-4" />
                  <span class="truncate">{{ service.name }}:{{ service.target_port }}</span>
                </Button>
                <span v-if="filteredServices(device.device_id).length === 0" class="text-sm text-muted-foreground">{{ t('device.noServices') }}</span>
              </div>
              <span v-else class="text-sm text-muted-foreground">{{ t('device.notLoaded') }}</span>
            </TableCell>
            <TableCell class="text-right">
              <Button variant="outline" size="sm" :disabled="serviceLoading[device.device_id]" @click="loadServices(device.device_id)">
                <ListTree class="size-4" />
                {{ t('device.service') }}
              </Button>
            </TableCell>
          </TableRow>
        </template>
        <template #cards>
          <div v-for="device in filteredDevices" :key="device.device_id" class="rounded-md border p-4">
            <div class="flex items-start justify-between gap-3">
              <div class="flex min-w-0 items-center gap-2">
                <HardDrive class="size-4 text-muted-foreground" />
                <p class="truncate font-medium">{{ device.name }}</p>
              </div>
              <Badge :variant="device.status === 'online' ? 'success' : 'secondary'">{{ formatDeviceStatus(device.status, t) }}</Badge>
            </div>
            <div class="mt-3">
              <InfoRow :label="t('common.deviceId')" :value="device.device_id" />
              <InfoRow :label="t('device.agentVersion')" :value="device.agent_version || '-'" />
            </div>
            <Button class="mt-3 w-full" variant="outline" size="sm" :disabled="serviceLoading[device.device_id]" @click="loadServices(device.device_id)">
              <ListTree class="size-4" />
              {{ t('device.loadServices') }}
            </Button>
            <div v-if="services[device.device_id]" class="mt-3 grid gap-2">
              <div v-for="service in filteredServices(device.device_id)" :key="service.service_id" class="rounded-md bg-muted p-3 text-sm">
                <div class="flex items-start justify-between gap-3">
                  <div class="min-w-0">
                    <p class="truncate font-medium">{{ service.name }}</p>
                    <p class="break-all text-muted-foreground">{{ service.protocol }} {{ service.target_host }}:{{ service.target_port }}</p>
                  </div>
                  <Button variant="outline" size="sm" @click="openSessionDialog(device, service)">
                    <Plus class="size-4" />
                    {{ t('device.session') }}
                  </Button>
                </div>
              </div>
              <EmptyState v-if="filteredServices(device.device_id).length === 0" :title="t('device.noServices')" />
            </div>
          </div>
        </template>
      </ResponsiveTable>
      </div>
    </PageSection>

    <Dialog :open="sessionOpen" @update:open="handleSessionOpenChange">
      <DialogContent class="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{{ t('device.createSessionTitle') }}</DialogTitle>
          <DialogDescription>
            {{ selectedDevice?.name }} / {{ selectedService?.name }}
          </DialogDescription>
        </DialogHeader>

        <form class="grid gap-4" @submit.prevent="createSession">
          <div class="grid gap-2">
            <Label for="session-controller-id">{{ t('device.controller') }}</Label>
            <Select v-model="sessionForm.client_id">
              <SelectTrigger id="session-controller-id"><SelectValue :placeholder="t('device.selectController')" /></SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="controller in availableControllers"
                  :key="controller.client_id"
                  :value="controller.client_id"
                >
                  {{ controller.name }} / {{ controller.client_id }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div class="grid gap-3 sm:grid-cols-2">
            <InfoRow :label="t('common.deviceId')" :value="sessionForm.device_id" />
            <InfoRow :label="t('common.serviceId')" :value="sessionForm.service_id" />
          </div>

          <div
            v-if="availableControllers.length === 0"
            class="rounded-md border border-warning/30 bg-warning/10 p-3 text-sm text-warning-foreground"
          >
            {{ t('device.noControllers') }}
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" :disabled="creatingSession" @click="resetSessionForm">
              {{ t('common.reset') }}
            </Button>
            <Button type="submit" :disabled="creatingSession || !sessionForm.client_id">
              <Loader2 v-if="creatingSession" class="animate-spin" />
              <KeyRound v-else class="size-4" />
              {{ t('device.createSession') }}
            </Button>
          </DialogFooter>
        </form>

        <div v-if="createdSession" class="grid gap-3 rounded-md border p-4">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="flex min-w-0 items-center gap-2">
              <KeyRound class="size-4 text-muted-foreground" />
              <p class="font-medium">{{ t('device.sessionResult') }}</p>
              <Badge class="min-w-0 max-w-full truncate" variant="outline">{{ createdSession.session_id }}</Badge>
            </div>
            <div class="flex shrink-0 items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                :disabled="!sessionClipboardText"
                @click="copySessionValue(sessionClipboardText, 'bundle')"
              >
                <Check v-if="copiedSessionField === 'bundle'" class="size-4" />
                <Copy v-else class="size-4" />
                {{ copiedSessionField === 'bundle' ? t('common.copied') : t('device.copyBundle') }}
              </Button>
              <Button variant="ghost" size="icon" :aria-label="t('device.closeSessionResult')" @click="dismissCreatedSession">
                <X class="size-4" />
                <span class="sr-only">{{ t('device.closeSessionResult') }}</span>
              </Button>
            </div>
          </div>
          <div class="grid gap-3 sm:grid-cols-2">
            <InfoRow :label="t('common.relay')" :value="createdSession.relay_addr" />
            <InfoRow :label="t('common.punch')" :value="createdSession.punch_addr" />
            <InfoRow :label="t('common.expiresAt')" :value="formatEpoch(createdSession.expire_at)" />
            <InfoRow :label="t('device.p2pCert')" :value="createdSession.agent_p2p_cert_der?.length ? t('device.included') : '-'" />
          </div>
          <div class="grid gap-2">
            <div class="flex items-center justify-between gap-2">
              <Label>{{ t('common.accessToken') }}</Label>
              <Button variant="outline" size="sm" @click="copySessionValue(createdSession.access_token, 'access')">
                <Check v-if="copiedSessionField === 'access'" class="size-4" />
                <Copy v-else class="size-4" />
                {{ copiedSessionField === 'access' ? t('common.copied') : t('common.copy') }}
              </Button>
            </div>
            <p class="max-h-28 overflow-auto break-all rounded-md bg-muted p-3 font-mono text-xs">{{ createdSession.access_token }}</p>
          </div>
          <div class="grid gap-2">
            <div class="flex items-center justify-between gap-2">
              <Label>{{ t('common.relayToken') }}</Label>
              <Button variant="outline" size="sm" @click="copySessionValue(createdSession.relay_token, 'relay')">
                <Check v-if="copiedSessionField === 'relay'" class="size-4" />
                <Copy v-else class="size-4" />
                {{ copiedSessionField === 'relay' ? t('common.copied') : t('common.copy') }}
              </Button>
            </div>
            <p class="max-h-28 overflow-auto break-all rounded-md bg-muted p-3 font-mono text-xs">{{ createdSession.relay_token }}</p>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  </main>
</template>
