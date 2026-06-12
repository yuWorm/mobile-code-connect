<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Check, Copy, Eye, KeyRound, Loader2, Pencil, Plus, RotateCw, Trash2, X } from 'lucide-vue-next'

import ConfirmAction from '@/components/layout/ConfirmAction.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
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
import { Switch } from '@/components/ui/switch'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { copyToClipboard } from '@/lib/control/clipboard'
import { formatBytes, formatDuration, formatEpoch } from '@/lib/control/format'
import { formatCredentialStatus, formatRelayHealth } from '@/lib/control/labels'
import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '@/lib/control/select-filter'
import type {
  RelayBootstrapExchangeResponse,
  RelayBootstrapResponse,
  RelayNode,
  RelaySessionSnapshot,
} from '@/lib/control/types'

const q = ref('')
const healthy = ref('')
const relaySort = ref('relay_id')
const credentialQ = ref('')
const credentialEnabled = ref('')
const credentialSort = ref('relay_id')
const relayOpen = ref(false)
const detailOpen = ref(false)
const credentialOpen = ref(false)
const bootstrapOpen = ref(false)
const exchangeOpen = ref(false)
const savingRelay = ref(false)
const savingCredential = ref(false)
const savingBootstrap = ref(false)
const exchangingBootstrap = ref(false)
const editingRelayId = ref('')
const selectedRelay = ref<RelayNode | null>(null)
const relaySessions = ref<RelaySessionSnapshot[]>([])
const relaySessionsLoading = ref(false)
const relaySessionsError = ref('')
const bootstrapResult = ref<RelayBootstrapResponse | null>(null)
const exchangeResult = ref<RelayBootstrapExchangeResponse | null>(null)
const bootstrapInstallMode = ref<'service' | 'no-service'>('service')
const copiedBootstrapField = ref<'token' | 'install-command' | null>(null)
const copiedExchangeField = ref<'control-token' | 'token-secret' | null>(null)
const { t } = useI18n()
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()

const relayForm = reactive({
  relay_id: '',
  relay_addr: '',
  capacity_streams: 128,
  healthy: true,
})
const credentialForm = reactive({
  relay_id: '',
  enabled: true,
})
const bootstrapForm = reactive({
  relay_id: '',
  control_url: defaultBootstrapControlUrl(),
  relay_addr: '',
  capacity_streams: 128,
  heartbeat_interval_sec: 30,
  ttl_sec: 900,
})
const exchangeForm = reactive({
  bootstrap_id: '',
  bootstrap_token: '',
})

const relayQuery = computed(() => ({
  q: q.value.trim(),
  healthy: healthy.value === '' ? undefined : healthy.value === 'true',
  limit: 100,
  sort: relaySort.value,
}))
const hasRelayFilters = computed(() =>
  q.value.trim() !== '' ||
  healthy.value !== '' ||
  relaySort.value !== 'relay_id',
)
const credentialQuery = computed(() => ({
  q: credentialQ.value.trim(),
  enabled: credentialEnabled.value === '' ? undefined : credentialEnabled.value === 'true',
  limit: 100,
  sort: credentialSort.value,
}))
const hasCredentialFilters = computed(() =>
  credentialQ.value.trim() !== '' ||
  credentialEnabled.value !== '' ||
  credentialSort.value !== 'relay_id',
)
const hasRelayForm = computed(() =>
  relayForm.relay_id.trim() !== '' &&
  relayForm.relay_addr.trim() !== '',
)
const hasCredentialForm = computed(() =>
  credentialForm.relay_id.trim() !== '',
)
const hasBootstrapForm = computed(() =>
  bootstrapForm.relay_id.trim() !== '' &&
  bootstrapForm.control_url.trim() !== '' &&
  bootstrapForm.relay_addr.trim() !== '' &&
  Number(bootstrapForm.capacity_streams) > 0 &&
  Number(bootstrapForm.heartbeat_interval_sec) > 0 &&
  Number(bootstrapForm.ttl_sec) > 0,
)
const hasExchangeForm = computed(() =>
  exchangeForm.bootstrap_id.trim() !== '' &&
  exchangeForm.bootstrap_token.trim() !== '',
)
const selectedBootstrapInstallCommand = computed(() => {
  if (bootstrapInstallMode.value === 'no-service') {
    return bootstrapResult.value?.no_service_install_command ?? ''
  }
  return bootstrapResult.value?.install_command ?? ''
})
const relays = useAsyncData(() => controlApi.relays(relayQuery.value))
const credentials = useAsyncData(() => controlApi.relayCredentials(credentialQuery.value))
watch([q, healthy, relaySort], () => relays.refresh())
watch([credentialQ, credentialEnabled, credentialSort], () => credentials.refresh())

function defaultBootstrapControlUrl() {
  if (typeof window === 'undefined') {
    return ''
  }
  return window.location.origin
}

function resetRelayFilters() {
  q.value = ''
  healthy.value = ''
  relaySort.value = 'relay_id'
}

function resetCredentialFilters() {
  credentialQ.value = ''
  credentialEnabled.value = ''
  credentialSort.value = 'relay_id'
}

function resetCredentialForm() {
  credentialForm.relay_id = ''
  credentialForm.enabled = true
}

function resetRelayForm() {
  editingRelayId.value = ''
  relayForm.relay_id = ''
  relayForm.relay_addr = ''
  relayForm.capacity_streams = 128
  relayForm.healthy = true
}

function resetBootstrapForm() {
  bootstrapForm.relay_id = ''
  bootstrapForm.control_url = defaultBootstrapControlUrl()
  bootstrapForm.relay_addr = ''
  bootstrapForm.capacity_streams = 128
  bootstrapForm.heartbeat_interval_sec = 30
  bootstrapForm.ttl_sec = 900
}

function resetExchangeForm() {
  exchangeForm.bootstrap_id = ''
  exchangeForm.bootstrap_token = ''
}

function clearBootstrapResult() {
  bootstrapResult.value = null
  bootstrapInstallMode.value = 'service'
  copiedBootstrapField.value = null
}

function clearExchangeResult() {
  exchangeResult.value = null
  copiedExchangeField.value = null
}

function openRelayCreator() {
  resetRelayForm()
  relayOpen.value = true
}

function handleRelayOpenChange(nextOpen: boolean) {
  if (savingRelay.value && !nextOpen) {
    return
  }
  relayOpen.value = nextOpen
  if (!nextOpen) {
    resetRelayForm()
  }
}

function handleCredentialOpenChange(nextOpen: boolean) {
  if (savingCredential.value && !nextOpen) {
    return
  }
  credentialOpen.value = nextOpen
  if (!nextOpen) {
    resetCredentialForm()
  }
}

function handleBootstrapOpenChange(nextOpen: boolean) {
  if (savingBootstrap.value && !nextOpen) {
    return
  }
  bootstrapOpen.value = nextOpen
  if (!nextOpen) {
    resetBootstrapForm()
  }
}

function handleExchangeOpenChange(nextOpen: boolean) {
  if (exchangingBootstrap.value && !nextOpen) {
    return
  }
  exchangeOpen.value = nextOpen
  if (!nextOpen) {
    resetExchangeForm()
  }
}

function openRelayEditor(relay: RelayNode) {
  editingRelayId.value = relay.relay_id
  relayForm.relay_id = relay.relay_id
  relayForm.relay_addr = relay.relay_addr
  relayForm.capacity_streams = relay.capacity_streams
  relayForm.healthy = relay.healthy
  relayOpen.value = true
}

function openRelayDetail(relay: RelayNode) {
  selectedRelay.value = relay
  detailOpen.value = true
  void loadRelaySessions(relay.relay_id)
}

function handleDetailOpenChange(nextOpen: boolean) {
  detailOpen.value = nextOpen
  if (!nextOpen) {
    selectedRelay.value = null
    relaySessions.value = []
    relaySessionsError.value = ''
  }
}

async function loadRelaySessions(relayId: string) {
  relaySessionsLoading.value = true
  relaySessionsError.value = ''
  try {
    const page = await controlApi.relaySessions(relayId, {
      limit: 100,
      sort: 'session_id',
    })
    relaySessions.value = page.items
  } catch (error) {
    relaySessions.value = []
    relaySessionsError.value = error instanceof Error ? error.message : t('relaySession.loadFailed')
  } finally {
    relaySessionsLoading.value = false
  }
}

function formatRelayHealthStatus(status: RelayNode['health_status']) {
  if (status === 'degraded') {
    return t('relay.healthStatusDegraded')
  }
  if (status === 'unhealthy') {
    return t('status.unhealthy')
  }
  return t('status.healthy')
}

function formatBoundState(bound: boolean) {
  return bound ? t('label.enabled') : t('label.disabled')
}

function formatRelaySessionState(state: string) {
  if (state === 'ready') {
    return t('relaySession.stateReady')
  }
  if (state === 'waiting') {
    return t('relaySession.stateWaiting')
  }
  if (state === 'closed') {
    return t('relaySession.stateClosed')
  }
  return state || '-'
}

function formatRelaySessionPeers(session: RelaySessionSnapshot) {
  const mobile = session.mobile_bound ? t('relaySession.mobileBound') : t('relaySession.mobileMissing')
  const agent = session.agent_bound ? t('relaySession.agentBound') : t('relaySession.agentMissing')
  return `${mobile} / ${agent}`
}

async function createBootstrap() {
  if (savingBootstrap.value || !hasBootstrapForm.value) {
    return
  }
  savingBootstrap.value = true
  try {
    await runWithToast(
      async () => {
        const result = await controlApi.createRelayBootstrap({
          relay_id: bootstrapForm.relay_id.trim(),
          control_url: bootstrapForm.control_url.trim(),
          relay_addr: bootstrapForm.relay_addr.trim(),
          admin_addr: '',
          capacity_streams: Number(bootstrapForm.capacity_streams),
          heartbeat_interval_sec: Number(bootstrapForm.heartbeat_interval_sec),
          ttl_sec: Number(bootstrapForm.ttl_sec),
        })
        bootstrapResult.value = result
        bootstrapInstallMode.value = 'service'
        copiedBootstrapField.value = null
        bootstrapOpen.value = false
        resetBootstrapForm()
      },
      {
        success: t('relayBootstrap.toast.created'),
        error: t('relayBootstrap.toast.createFailed'),
      },
    )
  } finally {
    savingBootstrap.value = false
  }
}

async function exchangeBootstrap() {
  if (exchangingBootstrap.value || !hasExchangeForm.value) {
    return
  }
  exchangingBootstrap.value = true
  try {
    await runWithToast(
      async () => {
        const result = await controlApi.exchangeRelayBootstrap(exchangeForm.bootstrap_id.trim(), {
          bootstrap_token: exchangeForm.bootstrap_token.trim(),
        })
        exchangeResult.value = result
        copiedExchangeField.value = null
        exchangeOpen.value = false
        resetExchangeForm()
        await credentials.refresh()
        await relays.refresh()
      },
      {
        success: t('relayBootstrap.toast.exchanged'),
        error: t('relayBootstrap.toast.exchangeFailed'),
      },
    )
  } finally {
    exchangingBootstrap.value = false
  }
}

async function saveRelay() {
  if (savingRelay.value || !hasRelayForm.value) {
    return
  }
  savingRelay.value = true
  try {
    await runWithToast(
      async () => {
        if (editingRelayId.value) {
          await controlApi.updateRelay(editingRelayId.value, {
            relay_id: relayForm.relay_id,
            relay_addr: relayForm.relay_addr,
            admin_addr: '',
            capacity_streams: Number(relayForm.capacity_streams),
            healthy: relayForm.healthy,
          })
        } else {
          await controlApi.registerRelay({
            relay_id: relayForm.relay_id,
            relay_addr: relayForm.relay_addr,
            admin_addr: '',
            capacity_streams: Number(relayForm.capacity_streams),
          })
        }
        relayOpen.value = false
        resetRelayForm()
        await relays.refresh()
      },
      {
        success: editingRelayId.value ? t('relay.toast.updated') : t('relay.toast.registered'),
        error: editingRelayId.value ? t('relay.toast.updateFailed') : t('relay.toast.registerFailed'),
      },
    )
  } finally {
    savingRelay.value = false
  }
}

async function createCredential() {
  if (savingCredential.value || !hasCredentialForm.value) {
    return
  }
  savingCredential.value = true
  try {
    await runWithToast(
      async () => {
        await controlApi.createRelayCredential({ ...credentialForm })
        credentialOpen.value = false
        resetCredentialForm()
        await credentials.refresh()
      },
      {
        success: t('relayCredential.toast.created'),
        error: t('relayCredential.toast.createFailed'),
      },
    )
  } finally {
    savingCredential.value = false
  }
}

async function removeRelay(relayId: string) {
  await runBusyAction(`remove-relay:${relayId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.removeRelay(relayId)
        await relays.refresh()
      },
      {
        success: t('relay.toast.removed'),
        error: t('relay.toast.removeFailed'),
      },
    )
  })
}

async function disconnectRelaySession(sessionId: string) {
  const relayId = selectedRelay.value?.relay_id
  if (!relayId) {
    return
  }
  await runBusyAction(`disconnect-relay-session:${sessionId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.disconnectRelaySession(relayId, sessionId)
        await loadRelaySessions(relayId)
        await relays.refresh()
      },
      {
        success: t('relaySession.toast.disconnectQueued'),
        error: t('relaySession.toast.disconnectFailed'),
      },
    )
  })
}

async function toggleCredential(relayId: string, enabled: boolean) {
  await runBusyAction(`toggle-credential:${relayId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.updateRelayCredentialStatus(relayId, { enabled: !enabled })
        await credentials.refresh()
      },
      {
        success: enabled ? t('relayCredential.toast.disabled') : t('relayCredential.toast.enabled'),
        error: t('relayCredential.toast.statusFailed'),
      },
    )
  })
}

async function rotateCredential(relayId: string) {
  await runBusyAction(`rotate-credential:${relayId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.rotateRelayCredential(relayId)
        await credentials.refresh()
      },
      {
        success: t('relayCredential.toast.rotated'),
        error: t('relayCredential.toast.rotateFailed'),
      },
    )
  })
}

async function copyBootstrapField(field: 'token' | 'install-command', value: string) {
  await copyToClipboard(value)
  copiedBootstrapField.value = field
  window.setTimeout(() => {
    if (copiedBootstrapField.value === field) {
      copiedBootstrapField.value = null
    }
  }, 1600)
}

async function copyExchangeField(field: 'control-token' | 'token-secret', value: string) {
  await copyToClipboard(value)
  copiedExchangeField.value = field
  window.setTimeout(() => {
    if (copiedExchangeField.value === field) {
      copiedExchangeField.value = null
    }
  }, 1600)
}
</script>

<template>
  <main class="page-container">
    <PageSection :title="t('section.admin.relays.nodesTitle')" :description="t('section.admin.relays.nodesDescription')">
      <template #actions>
        <Dialog :open="relayOpen" @update:open="handleRelayOpenChange">
          <DialogTrigger as-child>
            <Button @click="openRelayCreator"><Plus class="size-4" />{{ t('relay.register') }}</Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{{ editingRelayId ? t('relay.edit') : t('relay.register') }}</DialogTitle>
              <DialogDescription>{{ t('relay.dialogDescription') }}</DialogDescription>
            </DialogHeader>
            <form class="grid gap-4" @submit.prevent="saveRelay">
              <div class="grid gap-2">
                <Label for="relay-id">{{ t('common.relayId') }}</Label>
                <Input id="relay-id" v-model="relayForm.relay_id" required :disabled="!!editingRelayId" />
              </div>
              <div class="grid gap-2">
                <Label for="relay-addr">{{ t('relay.addr') }}</Label>
                <Input id="relay-addr" v-model="relayForm.relay_addr" required placeholder="relay.example.com:4433" />
              </div>
              <div class="grid gap-2">
                <Label for="relay-capacity-streams">{{ t('relay.capacityStreams') }}</Label>
                <Input id="relay-capacity-streams" v-model.number="relayForm.capacity_streams" required type="number" min="1" />
              </div>
              <div class="flex items-center justify-between rounded-md border p-3">
                <Label for="relay-healthy">{{ t('relay.healthy') }}</Label>
                <Switch id="relay-healthy" v-model:checked="relayForm.healthy" />
              </div>
              <DialogFooter>
                <Button type="submit" :disabled="savingRelay || !hasRelayForm">
                  <Loader2 v-if="savingRelay" class="animate-spin" />
                  {{ editingRelayId ? t('common.save') : t('common.register') }}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </template>

      <div class="grid gap-4">
        <SearchToolbar v-model="q" :placeholder="t('relay.searchPlaceholder')" :loading="relays.loading.value" @refresh="relays.refresh" />
        <div class="grid gap-3 md:grid-cols-[220px_220px_auto_1fr] md:items-center">
          <Select :model-value="selectFilterValue(healthy)" @update:model-value="healthy = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('relay.filterHealth')"><SelectValue :placeholder="t('relay.allHealth')" /></SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('relay.allHealth') }}</SelectItem>
              <SelectItem value="true">{{ formatRelayHealth(true, t) }}</SelectItem>
              <SelectItem value="false">{{ formatRelayHealth(false, t) }}</SelectItem>
            </SelectContent>
          </Select>
          <Select v-model="relaySort">
            <SelectTrigger :aria-label="t('relay.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="relay_id">{{ t('common.relayId') }}</SelectItem>
              <SelectItem value="healthy">{{ t('relay.sortHealth') }}</SelectItem>
              <SelectItem value="-capacity_streams">{{ t('relay.capacity') }}</SelectItem>
              <SelectItem value="-last_seen_epoch_sec">{{ t('relay.lastSeen') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasRelayFilters" @click="resetRelayFilters">
            {{ t('common.reset') }}
          </Button>
          <p class="text-sm text-muted-foreground md:text-right">
            {{ t('relay.total', { total: relays.data.value?.total ?? 0 }) }}
          </p>
        </div>
        <ResponsiveTable :items="relays.data.value?.items ?? []" :loading="relays.loading.value" :error="relays.error.value" :empty-title="t('relay.empty')" @retry="relays.refresh">
          <template #head>
            <TableRow>
              <TableHead>{{ t('common.relay') }}</TableHead>
              <TableHead>{{ t('relay.address') }}</TableHead>
              <TableHead>{{ t('relay.capacity') }}</TableHead>
              <TableHead>{{ t('relay.load') }}</TableHead>
              <TableHead>{{ t('relay.healthy') }}</TableHead>
              <TableHead>{{ t('relay.lastSeen') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="relay in relays.data.value?.items ?? []" :key="relay.relay_id">
              <TableCell class="font-medium">{{ relay.relay_id }}</TableCell>
              <TableCell>{{ relay.relay_addr }}</TableCell>
              <TableCell>{{ relay.capacity_streams }}</TableCell>
              <TableCell>{{ relay.active_sessions }} / {{ relay.active_streams }}</TableCell>
              <TableCell>
                <Badge :variant="relay.healthy ? 'success' : 'secondary'">{{ formatRelayHealth(relay.healthy, t) }}</Badge>
                <div v-if="relay.health_reason" class="mt-1 text-xs text-muted-foreground">{{ relay.health_reason }}</div>
              </TableCell>
              <TableCell>{{ formatEpoch(relay.last_seen_epoch_sec) }}</TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button variant="outline" size="sm" :disabled="hasBusyAction" @click="openRelayDetail(relay)">
                    <Eye class="size-4" />
                    {{ t('common.details') }}
                  </Button>
                  <Button variant="outline" size="sm" :disabled="hasBusyAction" @click="openRelayEditor(relay)">
                    <Pencil class="size-4" />
                    {{ t('common.edit') }}
                  </Button>
                  <ConfirmAction
                    :title="t('relay.removeTitle')"
                    :description="t('relay.removeDescription', { id: relay.relay_id })"
                    :confirm-text="t('common.remove')"
                    variant="outline"
                    :icon="Trash2"
                    :disabled="hasBusyAction"
                    :loading="isBusy('remove-relay', relay.relay_id)"
                    @confirm="removeRelay(relay.relay_id)"
                  >
                    {{ t('common.remove') }}
                  </ConfirmAction>
                </div>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="relay in relays.data.value?.items ?? []" :key="relay.relay_id" class="rounded-md border p-4">
              <div class="flex items-start justify-between gap-3">
                <p class="font-medium">{{ relay.relay_id }}</p>
                <Badge :variant="relay.healthy ? 'success' : 'secondary'">{{ formatRelayHealth(relay.healthy, t) }}</Badge>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('relay.addr')" :value="relay.relay_addr" />
                <InfoRow :label="t('relay.capacity')" :value="relay.capacity_streams" />
                <InfoRow :label="t('relay.load')" :value="`${relay.active_sessions} / ${relay.active_streams}`" />
                <InfoRow :label="t('relay.totalBytes')" :value="relay.total_bytes" />
                <InfoRow v-if="relay.relay_version" :label="t('relay.version')" :value="relay.relay_version" />
                <InfoRow v-if="relay.health_reason" :label="t('relay.healthReason')" :value="relay.health_reason" />
                <InfoRow :label="t('relay.lastSeen')" :value="formatEpoch(relay.last_seen_epoch_sec)" />
              </div>
              <div class="mt-3 grid grid-cols-2 gap-2">
                <Button variant="outline" size="sm" :disabled="hasBusyAction" @click="openRelayDetail(relay)">
                  <Eye class="size-4" />
                  {{ t('common.details') }}
                </Button>
                <Button variant="outline" size="sm" :disabled="hasBusyAction" @click="openRelayEditor(relay)">
                  <Pencil class="size-4" />
                  {{ t('common.edit') }}
                </Button>
                <ConfirmAction
                  :title="t('relay.removeTitle')"
                  :description="t('relay.removeDescription', { id: relay.relay_id })"
                  :confirm-text="t('common.remove')"
                  variant="outline"
                  :icon="Trash2"
                  :disabled="hasBusyAction"
                  :loading="isBusy('remove-relay', relay.relay_id)"
                  @confirm="removeRelay(relay.relay_id)"
                >
                  {{ t('common.remove') }}
                </ConfirmAction>
              </div>
            </div>
          </template>
        </ResponsiveTable>

        <Dialog :open="detailOpen" @update:open="handleDetailOpenChange">
          <DialogContent v-if="selectedRelay" class="sm:max-w-3xl">
            <DialogHeader>
              <DialogTitle>{{ t('relay.detailTitle') }}</DialogTitle>
              <DialogDescription>{{ t('relay.detailDescription') }}</DialogDescription>
            </DialogHeader>
            <div class="grid gap-5">
              <div class="flex flex-wrap items-center justify-between gap-3 rounded-md border p-4">
                <div>
                  <p class="font-medium">{{ selectedRelay.relay_id }}</p>
                  <p class="mt-1 text-sm text-muted-foreground">{{ selectedRelay.relay_addr }}</p>
                </div>
                <Badge :variant="selectedRelay.healthy ? 'success' : 'secondary'">
                  {{ formatRelayHealthStatus(selectedRelay.health_status) }}
                </Badge>
              </div>

              <div class="grid gap-3 sm:grid-cols-2">
                <InfoRow :label="t('relay.healthStatus')" :value="formatRelayHealthStatus(selectedRelay.health_status)" />
                <InfoRow :label="t('relay.healthReason')" :value="selectedRelay.health_reason || '-'" />
                <InfoRow :label="t('relay.dataPlaneBound')" :value="formatBoundState(selectedRelay.data_plane_bound)" />
                <InfoRow :label="t('relay.lastSeen')" :value="formatEpoch(selectedRelay.last_seen_epoch_sec)" />
                <InfoRow :label="t('relay.lastHealthReport')" :value="formatEpoch(selectedRelay.last_health_report_epoch_sec)" />
                <InfoRow :label="t('relay.version')" :value="selectedRelay.relay_version || '-'" />
                <InfoRow :label="t('relay.uptime')" :value="formatDuration(selectedRelay.uptime_sec)" />
              </div>

              <div class="grid gap-3 sm:grid-cols-2">
                <InfoRow :label="t('relay.addr')" :value="selectedRelay.relay_addr" />
                <InfoRow :label="t('relay.capacity')" :value="selectedRelay.capacity_streams" />
                <InfoRow :label="t('relay.load')" :value="`${selectedRelay.active_sessions} / ${selectedRelay.active_streams}`" />
              </div>

              <div class="grid gap-3 sm:grid-cols-3">
                <InfoRow :label="t('relay.traffic')" :value="formatBytes(selectedRelay.total_bytes)" />
                <InfoRow :label="t('relay.uplink')" :value="formatBytes(selectedRelay.total_uplink_bytes)" />
                <InfoRow :label="t('relay.downlink')" :value="formatBytes(selectedRelay.total_downlink_bytes)" />
              </div>

              <Card>
                <CardHeader class="flex-row items-center justify-between gap-3">
                  <CardTitle>{{ t('relay.sessionsTitle') }}</CardTitle>
                  <Button variant="outline" size="sm" :disabled="relaySessionsLoading" @click="loadRelaySessions(selectedRelay.relay_id)">
                    <Loader2 v-if="relaySessionsLoading" class="animate-spin" />
                    {{ t('common.refresh') }}
                  </Button>
                </CardHeader>
                <CardContent>
                  <div v-if="relaySessionsLoading" class="py-6 text-sm text-muted-foreground">
                    {{ t('common.loading') }}
                  </div>
                  <div v-else-if="relaySessionsError" class="rounded-md border border-destructive/30 p-3 text-sm text-destructive">
                    {{ relaySessionsError }}
                  </div>
                  <div v-else-if="relaySessions.length === 0" class="rounded-md border border-dashed p-3 text-sm text-muted-foreground">
                    {{ t('relaySession.empty') }}
                  </div>
                  <Table v-else>
                    <TableHeader>
                      <TableRow>
                        <TableHead>{{ t('common.session') }}</TableHead>
                        <TableHead>{{ t('relaySession.state') }}</TableHead>
                        <TableHead>{{ t('relaySession.peers') }}</TableHead>
                        <TableHead>{{ t('relaySession.activeStreams') }}</TableHead>
                        <TableHead>{{ t('relaySession.bytes') }}</TableHead>
                        <TableHead>{{ t('relay.lastSeen') }}</TableHead>
                        <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      <TableRow v-for="session in relaySessions" :key="session.session_id">
                        <TableCell class="font-medium">{{ session.session_id }}</TableCell>
                        <TableCell>
                          <Badge :variant="session.state === 'ready' ? 'success' : 'secondary'">
                            {{ formatRelaySessionState(session.state) }}
                          </Badge>
                        </TableCell>
                        <TableCell>{{ formatRelaySessionPeers(session) }}</TableCell>
                        <TableCell>{{ session.stats.active_streams }}</TableCell>
                        <TableCell>{{ formatBytes(session.stats.total_bytes) }}</TableCell>
                        <TableCell>{{ formatEpoch(session.last_seen_epoch_sec) }}</TableCell>
                        <TableCell class="text-right">
                          <ConfirmAction
                            :title="t('relaySession.disconnectTitle')"
                            :description="t('relaySession.disconnectDescription', { id: session.session_id })"
                            :confirm-text="t('relaySession.disconnect')"
                            variant="outline"
                            :icon="X"
                            :disabled="hasBusyAction || session.state === 'closed'"
                            :loading="isBusy('disconnect-relay-session', session.session_id)"
                            @confirm="disconnectRelaySession(session.session_id)"
                          >
                            {{ t('relaySession.disconnect') }}
                          </ConfirmAction>
                        </TableCell>
                      </TableRow>
                    </TableBody>
                  </Table>
                </CardContent>
              </Card>
            </div>
          </DialogContent>
        </Dialog>
      </div>
    </PageSection>

    <PageSection :title="t('section.admin.relays.bootstrapTitle')" :description="t('section.admin.relays.bootstrapDescription')">
      <template #actions>
        <div class="flex flex-wrap gap-2">
          <Dialog :open="bootstrapOpen" @update:open="handleBootstrapOpenChange">
            <DialogTrigger as-child>
              <Button variant="outline" @click="resetBootstrapForm">
                <Plus class="size-4" />
                {{ t('relayBootstrap.create') }}
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>{{ t('relayBootstrap.dialogTitle') }}</DialogTitle>
                <DialogDescription>{{ t('relayBootstrap.dialogDescription') }}</DialogDescription>
              </DialogHeader>
              <form class="grid gap-4" @submit.prevent="createBootstrap">
                <div class="grid gap-2">
                  <Label for="bootstrap-relay-id">{{ t('common.relayId') }}</Label>
                  <Input id="bootstrap-relay-id" v-model="bootstrapForm.relay_id" required placeholder="relay_prod_001" />
                </div>
                <div class="grid gap-2">
                  <Label for="bootstrap-control-url">{{ t('relayBootstrap.controlUrl') }}</Label>
                  <Input id="bootstrap-control-url" v-model="bootstrapForm.control_url" required placeholder="https://control.example.com" />
                </div>
                <div class="grid gap-2">
                  <Label for="bootstrap-relay-addr">{{ t('relay.addr') }}</Label>
                  <Input id="bootstrap-relay-addr" v-model="bootstrapForm.relay_addr" required placeholder="relay.example.com:4433" />
                </div>
                <div class="grid gap-3 sm:grid-cols-3">
                  <div class="grid gap-2">
                    <Label for="bootstrap-capacity-streams">{{ t('relay.capacityStreams') }}</Label>
                    <Input id="bootstrap-capacity-streams" v-model.number="bootstrapForm.capacity_streams" required type="number" min="1" />
                  </div>
                  <div class="grid gap-2">
                    <Label for="bootstrap-heartbeat-interval">{{ t('relayBootstrap.heartbeatInterval') }}</Label>
                    <Input id="bootstrap-heartbeat-interval" v-model.number="bootstrapForm.heartbeat_interval_sec" required type="number" min="1" />
                  </div>
                  <div class="grid gap-2">
                    <Label for="bootstrap-ttl">{{ t('relayBootstrap.ttl') }}</Label>
                    <Input id="bootstrap-ttl" v-model.number="bootstrapForm.ttl_sec" required type="number" min="1" />
                  </div>
                </div>
                <DialogFooter>
                  <Button type="button" variant="outline" :disabled="savingBootstrap" @click="resetBootstrapForm">
                    {{ t('common.reset') }}
                  </Button>
                  <Button type="submit" :disabled="savingBootstrap || !hasBootstrapForm">
                    <Loader2 v-if="savingBootstrap" class="animate-spin" />
                    {{ t('relayBootstrap.create') }}
                  </Button>
                </DialogFooter>
              </form>
            </DialogContent>
          </Dialog>

          <Dialog :open="exchangeOpen" @update:open="handleExchangeOpenChange">
            <DialogTrigger as-child>
              <Button variant="outline" @click="resetExchangeForm">
                <KeyRound class="size-4" />
                {{ t('relayBootstrap.exchange') }}
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>{{ t('relayBootstrap.exchangeTitle') }}</DialogTitle>
                <DialogDescription>{{ t('relayBootstrap.exchangeDescription') }}</DialogDescription>
              </DialogHeader>
              <form class="grid gap-4" @submit.prevent="exchangeBootstrap">
                <div class="grid gap-2">
                  <Label for="exchange-bootstrap-id">{{ t('relayBootstrap.bootstrapId') }}</Label>
                  <Input id="exchange-bootstrap-id" v-model="exchangeForm.bootstrap_id" required placeholder="rb_..." />
                </div>
                <div class="grid gap-2">
                  <Label for="exchange-bootstrap-token">{{ t('relayBootstrap.bootstrapToken') }}</Label>
                  <Input id="exchange-bootstrap-token" v-model="exchangeForm.bootstrap_token" required placeholder="rbt_..." />
                </div>
                <DialogFooter>
                  <Button type="button" variant="outline" :disabled="exchangingBootstrap" @click="resetExchangeForm">
                    {{ t('common.reset') }}
                  </Button>
                  <Button type="submit" :disabled="exchangingBootstrap || !hasExchangeForm">
                    <Loader2 v-if="exchangingBootstrap" class="animate-spin" />
                    {{ t('relayBootstrap.exchange') }}
                  </Button>
                </DialogFooter>
              </form>
            </DialogContent>
          </Dialog>
        </div>
      </template>

      <div class="grid gap-4 xl:grid-cols-2">
        <Card v-if="bootstrapResult?.bootstrap_token">
          <CardHeader class="flex-row items-center justify-between gap-3">
            <CardTitle>{{ t('relayBootstrap.resultTitle') }}</CardTitle>
            <Button variant="ghost" size="icon" :aria-label="t('relayBootstrap.closeResult')" @click="clearBootstrapResult">
              <X class="size-4" />
              <span class="sr-only">{{ t('relayBootstrap.closeResult') }}</span>
            </Button>
          </CardHeader>
          <CardContent class="grid gap-3">
            <div class="grid gap-2 sm:grid-cols-2">
              <InfoRow :label="t('relayBootstrap.bootstrapId')" :value="bootstrapResult.bootstrap_id" />
              <InfoRow :label="t('common.relayId')" :value="bootstrapResult.relay_id" />
              <InfoRow :label="t('relayBootstrap.controlUrl')" :value="bootstrapResult.control_url" />
              <InfoRow :label="t('common.expiresAt')" :value="formatEpoch(bootstrapResult.expires_epoch_sec)" />
            </div>
            <div class="grid gap-2">
              <div class="flex items-center justify-between gap-3">
                <Label>{{ t('relayBootstrap.bootstrapToken') }}</Label>
                <Button variant="outline" size="sm" @click="copyBootstrapField('token', bootstrapResult.bootstrap_token)">
                  <Check v-if="copiedBootstrapField === 'token'" class="size-4" />
                  <Copy v-else class="size-4" />
                  {{ copiedBootstrapField === 'token' ? t('common.copied') : t('relayBootstrap.copyToken') }}
                </Button>
              </div>
              <p class="max-h-28 overflow-auto break-all rounded-md bg-muted p-3 font-mono text-xs">{{ bootstrapResult.bootstrap_token }}</p>
            </div>
            <Tabs v-model="bootstrapInstallMode" class="grid gap-3">
              <TabsList class="grid w-full grid-cols-2">
                <TabsTrigger value="service">{{ t('relayBootstrap.installModeService') }}</TabsTrigger>
                <TabsTrigger value="no-service">{{ t('relayBootstrap.installModeNoService') }}</TabsTrigger>
              </TabsList>
              <div class="flex items-center justify-between gap-3">
                <Label>{{ t('relayBootstrap.installCommand') }}</Label>
                <Button variant="outline" size="sm" :disabled="!selectedBootstrapInstallCommand" @click="copyBootstrapField('install-command', selectedBootstrapInstallCommand)">
                  <Check v-if="copiedBootstrapField === 'install-command'" class="size-4" />
                  <Copy v-else class="size-4" />
                  {{ copiedBootstrapField === 'install-command' ? t('common.copied') : t('relayBootstrap.copyInstallCommand') }}
                </Button>
              </div>
              <p class="max-h-28 overflow-auto break-all rounded-md bg-muted p-3 font-mono text-xs">{{ selectedBootstrapInstallCommand }}</p>
            </Tabs>
          </CardContent>
        </Card>

        <Card v-if="exchangeResult">
          <CardHeader class="flex-row items-center justify-between gap-3">
            <CardTitle>{{ t('relayBootstrap.exchangeResultTitle') }}</CardTitle>
            <Button variant="ghost" size="icon" :aria-label="t('relayBootstrap.closeExchangeResult')" @click="clearExchangeResult">
              <X class="size-4" />
              <span class="sr-only">{{ t('relayBootstrap.closeExchangeResult') }}</span>
            </Button>
          </CardHeader>
          <CardContent class="grid gap-3">
            <div class="grid gap-2 sm:grid-cols-2">
              <InfoRow :label="t('common.relayId')" :value="exchangeResult.relay_id" />
              <InfoRow :label="t('relayBootstrap.controlUrl')" :value="exchangeResult.control_url" />
              <InfoRow :label="t('relay.addr')" :value="exchangeResult.relay_addr" />
              <InfoRow :label="t('relay.capacityStreams')" :value="exchangeResult.capacity_streams" />
              <InfoRow :label="t('relayBootstrap.heartbeatInterval')" :value="formatDuration(exchangeResult.heartbeat_interval_sec)" />
            </div>
            <div class="grid gap-2">
              <div class="flex items-center justify-between gap-3">
                <Label>{{ t('relayBootstrap.controlToken') }}</Label>
                <Button variant="outline" size="sm" @click="copyExchangeField('control-token', exchangeResult.control_token)">
                  <Check v-if="copiedExchangeField === 'control-token'" class="size-4" />
                  <Copy v-else class="size-4" />
                  {{ copiedExchangeField === 'control-token' ? t('common.copied') : t('common.copy') }}
                </Button>
              </div>
              <p class="max-h-28 overflow-auto break-all rounded-md bg-muted p-3 font-mono text-xs">{{ exchangeResult.control_token }}</p>
            </div>
            <div class="grid gap-2">
              <div class="flex items-center justify-between gap-3">
                <Label>{{ t('relayBootstrap.tokenSecret') }}</Label>
                <Button variant="outline" size="sm" @click="copyExchangeField('token-secret', exchangeResult.token_secret)">
                  <Check v-if="copiedExchangeField === 'token-secret'" class="size-4" />
                  <Copy v-else class="size-4" />
                  {{ copiedExchangeField === 'token-secret' ? t('common.copied') : t('common.copy') }}
                </Button>
              </div>
              <p class="max-h-28 overflow-auto break-all rounded-md bg-muted p-3 font-mono text-xs">{{ exchangeResult.token_secret }}</p>
            </div>
          </CardContent>
        </Card>

        <div v-if="!bootstrapResult && !exchangeResult" class="rounded-md border border-dashed p-4 text-sm text-muted-foreground xl:col-span-2">
          {{ t('relayBootstrap.emptyResult') }}
        </div>
      </div>
    </PageSection>

    <PageSection :title="t('section.admin.relays.credentialsTitle')" :description="t('section.admin.relays.credentialsDescription')">
      <template #actions>
        <Dialog :open="credentialOpen" @update:open="handleCredentialOpenChange">
          <DialogTrigger as-child>
            <Button variant="outline" @click="resetCredentialForm"><KeyRound class="size-4" />{{ t('relayCredential.create') }}</Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{{ t('relayCredential.dialogTitle') }}</DialogTitle>
              <DialogDescription>{{ t('relayCredential.dialogDescription') }}</DialogDescription>
            </DialogHeader>
            <form class="grid gap-4" @submit.prevent="createCredential">
              <div class="grid gap-2">
                <Label for="credential-relay-id">{{ t('common.relayId') }}</Label>
                <Input id="credential-relay-id" v-model="credentialForm.relay_id" required />
              </div>
              <div class="flex items-center justify-between rounded-md border p-3">
                <Label for="credential-enabled">{{ t('common.enable') }}</Label>
                <Switch id="credential-enabled" v-model:checked="credentialForm.enabled" />
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" :disabled="savingCredential" @click="resetCredentialForm">
                  {{ t('common.reset') }}
                </Button>
                <Button type="submit" :disabled="savingCredential || !hasCredentialForm">
                  <Loader2 v-if="savingCredential" class="animate-spin" />
                  {{ t('common.create') }}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </template>
      <div class="grid gap-4">
        <SearchToolbar v-model="credentialQ" :placeholder="t('relayCredential.searchPlaceholder')" :loading="credentials.loading.value" @refresh="credentials.refresh" />
        <div class="grid gap-3 md:grid-cols-[220px_220px_auto_1fr] md:items-center">
          <Select :model-value="selectFilterValue(credentialEnabled)" @update:model-value="credentialEnabled = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('relayCredential.statusFilter')"><SelectValue :placeholder="t('common.allStatus')" /></SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('common.allStatus') }}</SelectItem>
              <SelectItem value="true">{{ formatCredentialStatus(true, t) }}</SelectItem>
              <SelectItem value="false">{{ formatCredentialStatus(false, t) }}</SelectItem>
            </SelectContent>
          </Select>
          <Select v-model="credentialSort">
            <SelectTrigger :aria-label="t('relayCredential.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="relay_id">{{ t('common.relayId') }}</SelectItem>
              <SelectItem value="enabled">{{ t('relayCredential.sortEnabled') }}</SelectItem>
              <SelectItem value="-token_version">{{ t('credential.tokenVersion') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasCredentialFilters" @click="resetCredentialFilters">
            {{ t('common.reset') }}
          </Button>
          <p class="text-sm text-muted-foreground md:text-right">
            {{ t('relayCredential.total', { total: credentials.data.value?.total ?? 0 }) }}
          </p>
        </div>
        <ResponsiveTable :items="credentials.data.value?.items ?? []" :loading="credentials.loading.value" :error="credentials.error.value" :empty-title="t('relayCredential.empty')" @retry="credentials.refresh">
          <template #head>
            <TableRow>
              <TableHead>{{ t('common.relayId') }}</TableHead>
              <TableHead>{{ t('credential.version') }}</TableHead>
              <TableHead>{{ t('common.status') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="credential in credentials.data.value?.items ?? []" :key="credential.relay_id">
              <TableCell class="font-medium">{{ credential.relay_id }}</TableCell>
              <TableCell>{{ credential.token_version }}</TableCell>
              <TableCell><Badge :variant="credential.enabled ? 'success' : 'secondary'">{{ formatCredentialStatus(credential.enabled, t) }}</Badge></TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="hasBusyAction"
                    @click="toggleCredential(credential.relay_id, credential.enabled)"
                  >
                    <Loader2 v-if="isBusy('toggle-credential', credential.relay_id)" class="animate-spin" />
                    {{ credential.enabled ? t('common.disable') : t('common.enable') }}
                  </Button>
                  <ConfirmAction
                    :title="t('relayCredential.rotateTitle')"
                    :description="t('relayCredential.rotateDescription', { id: credential.relay_id })"
                    :confirm-text="t('common.rotate')"
                    variant="outline"
                    :icon="RotateCw"
                    :disabled="hasBusyAction"
                    :loading="isBusy('rotate-credential', credential.relay_id)"
                    @confirm="rotateCredential(credential.relay_id)"
                  >
                    {{ t('common.rotate') }}
                  </ConfirmAction>
                </div>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="credential in credentials.data.value?.items ?? []" :key="credential.relay_id" class="rounded-md border p-4">
              <div class="flex items-start justify-between gap-3">
                <p class="font-medium">{{ credential.relay_id }}</p>
                <Badge :variant="credential.enabled ? 'success' : 'secondary'">{{ formatCredentialStatus(credential.enabled, t) }}</Badge>
              </div>
              <InfoRow class="mt-3" :label="t('credential.tokenVersion')" :value="credential.token_version" />
              <div class="mt-3 grid grid-cols-2 gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="hasBusyAction"
                  @click="toggleCredential(credential.relay_id, credential.enabled)"
                >
                  <Loader2 v-if="isBusy('toggle-credential', credential.relay_id)" class="animate-spin" />
                  {{ credential.enabled ? t('common.disable') : t('common.enable') }}
                </Button>
                <ConfirmAction
                  :title="t('relayCredential.rotateTitle')"
                  :description="t('relayCredential.rotateDescription', { id: credential.relay_id })"
                  :confirm-text="t('common.rotate')"
                  variant="outline"
                  :icon="RotateCw"
                  :disabled="hasBusyAction"
                  :loading="isBusy('rotate-credential', credential.relay_id)"
                  @confirm="rotateCredential(credential.relay_id)"
                >
                  {{ t('common.rotate') }}
                </ConfirmAction>
              </div>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>
  </main>
</template>
