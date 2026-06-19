<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Check, Copy, ExternalLink, KeyRound, Loader2, RotateCw, Server, X } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import ConfirmAction from '@/components/layout/ConfirmAction.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { copyToClipboard } from '@/lib/control/clipboard'
import { formatDuration, formatEpoch } from '@/lib/control/format'
import { formatCredentialStatus, formatDeviceAuthStatus } from '@/lib/control/labels'
import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '@/lib/control/select-filter'
import type { DeviceServerAuthStartResponse, ServerCredentialResponse } from '@/lib/control/types'

const q = ref('')
const deviceId = ref('')
const enabled = ref('')
const sort = ref('-created_epoch_sec')
const { t } = useI18n()
const credentialsQuery = computed(() => ({
  q: q.value.trim(),
  device_id: deviceId.value.trim(),
  enabled: enabled.value === '' ? undefined : enabled.value === 'true',
  limit: 100,
  sort: sort.value,
}))
const hasCredentialFilters = computed(() =>
  q.value.trim() !== '' ||
  deviceId.value.trim() !== '' ||
  enabled.value !== '' ||
  sort.value !== '-created_epoch_sec',
)
const credentials = useAsyncData(() => controlApi.serverCredentials(credentialsQuery.value))
const rotated = ref<ServerCredentialResponse | null>(null)
const startingAuth = ref(false)
const pollingAuth = ref(false)
const approvingAuth = ref(false)
const denyingAuth = ref(false)
const copiedField = ref<'token' | 'code' | 'url' | null>(null)
const deviceAuth = ref<DeviceServerAuthStartResponse | null>(null)
const deviceAuthStatus = ref('')
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
const authForm = reactive({
  device_name: '',
  server_public_key: '',
})
const hasDeviceAuthForm = computed(() =>
  authForm.device_name.trim() !== '' &&
  authForm.server_public_key.trim() !== '',
)
const deviceAuthBusy = computed(() =>
  startingAuth.value || pollingAuth.value || approvingAuth.value || denyingAuth.value,
)
watch([q, deviceId, enabled, sort], () => credentials.refresh())
const approvalUrl = computed(() => {
  if (!deviceAuth.value) {
    return ''
  }
  return new URL(deviceAuth.value.verification_uri_complete, window.location.origin).toString()
})

function resetCredentialFilters() {
  q.value = ''
  deviceId.value = ''
  enabled.value = ''
  sort.value = '-created_epoch_sec'
}

function resetDeviceAuthForm() {
  authForm.device_name = ''
  authForm.server_public_key = ''
  deviceAuth.value = null
  deviceAuthStatus.value = ''
  copiedField.value = null
}

async function toggleCredential(credentialId: string, nextEnabled: boolean) {
  await runBusyAction(`toggle:${credentialId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.updateServerCredentialStatus(credentialId, { enabled: nextEnabled })
        await credentials.refresh()
      },
      {
        success: nextEnabled ? t('credential.toast.enabled') : t('credential.toast.disabled'),
        error: t('credential.toast.statusFailed'),
      },
    )
  })
}

async function rotateCredential(credentialId: string) {
  await runBusyAction(`rotate:${credentialId}`, async () => {
    await runWithToast(
      async () => {
        rotated.value = await controlApi.rotateServerCredential(credentialId)
        copiedField.value = null
        await credentials.refresh()
      },
      {
        success: t('credential.toast.rotated'),
        error: t('credential.toast.rotateFailed'),
      },
    )
  })
}

async function startDeviceAuth() {
  if (!hasDeviceAuthForm.value || deviceAuthBusy.value) {
    return
  }
  startingAuth.value = true
  deviceAuth.value = null
  deviceAuthStatus.value = ''
  copiedField.value = null
  try {
    await runWithToast(
      async () => {
        deviceAuth.value = await controlApi.startDeviceServerAuth({
          device_name: authForm.device_name.trim(),
          server_public_key: authForm.server_public_key.trim(),
        })
      },
      {
        success: t('deviceAuth.toast.started'),
        error: t('deviceAuth.toast.startFailed'),
      },
    )
  } finally {
    startingAuth.value = false
  }
}

async function approveDeviceAuth() {
  if (!deviceAuth.value || deviceAuthBusy.value) {
    return
  }
  approvingAuth.value = true
  try {
    await runWithToast(
      async () => {
        const approval = await controlApi.approveDeviceServerAuth(deviceAuth.value!.user_code)
        deviceAuthStatus.value = approval.status
      },
      {
        success: t('deviceAuth.toast.approved'),
        error: t('deviceAuth.toast.approveFailed'),
      },
    )
  } finally {
    approvingAuth.value = false
  }
}

async function denyDeviceAuth() {
  if (!deviceAuth.value || deviceAuthBusy.value) {
    return
  }
  denyingAuth.value = true
  try {
    await runWithToast(
      async () => {
        const denial = await controlApi.denyDeviceServerAuth(deviceAuth.value!.user_code)
        deviceAuthStatus.value = denial.status
      },
      {
        success: t('deviceAuth.toast.denied'),
        error: t('deviceAuth.toast.denyFailed'),
      },
    )
  } finally {
    denyingAuth.value = false
  }
}

async function pollDeviceAuth() {
  if (!deviceAuth.value || deviceAuthBusy.value) {
    return
  }
  pollingAuth.value = true
  try {
    await runWithToast(
      async () => {
        const poll = await controlApi.pollDeviceServerAuth({
          device_code: deviceAuth.value!.device_code,
          server_public_key: authForm.server_public_key.trim(),
        })
        deviceAuthStatus.value = poll.status
        if (poll.credential) {
          rotated.value = poll.credential
          resetDeviceAuthForm()
          await credentials.refresh()
          return 'issued'
        }
        if (poll.status === 'authorization_pending' || poll.status === 'slow_down') {
          toast(t('deviceAuth.toast.pending'), {
            description: t('deviceAuth.toast.pendingDescription', { duration: formatDuration(poll.interval) }),
          })
          return 'pending'
        }
        throw new Error(t('deviceAuth.statusError', { status: poll.status }))
      },
      {
        success: (result) => (result === 'issued' ? t('deviceAuth.toast.issued') : ''),
        error: t('deviceAuth.toast.pollFailed'),
      },
    )
  } finally {
    pollingAuth.value = false
  }
}

function openApprovalUrl() {
  if (approvalUrl.value && !deviceAuthBusy.value) {
    window.open(approvalUrl.value, '_blank', 'noopener,noreferrer')
  }
}

async function copyValue(value: string, field: 'token' | 'code' | 'url') {
  await copyToClipboard(value)
  copiedField.value = field
  window.setTimeout(() => {
    if (copiedField.value === field) {
      copiedField.value = null
    }
  }, 1600)
}

async function copyToken() {
  if (rotated.value) {
    await copyValue(rotated.value.server_token, 'token')
  }
}

function dismissRotatedToken() {
  rotated.value = null
  copiedField.value = null
}

function dismissDeviceAuth() {
  deviceAuth.value = null
  deviceAuthStatus.value = ''
  copiedField.value = null
}

</script>

<template>
  <main class="page-container">
    <Card v-if="rotated">
      <CardHeader class="flex-row items-center justify-between gap-3">
        <CardTitle>{{ t('credential.newServerToken') }}</CardTitle>
        <Button variant="ghost" size="icon" :aria-label="t('credential.closeServerToken')" @click="dismissRotatedToken">
          <X class="size-4" />
          <span class="sr-only">{{ t('credential.closeServerToken') }}</span>
        </Button>
      </CardHeader>
      <CardContent class="grid gap-3">
        <p class="max-h-28 overflow-auto break-all rounded-md bg-muted p-3 font-mono text-xs">{{ rotated.server_token }}</p>
        <div class="flex flex-wrap items-center gap-2">
          <Badge variant="outline">{{ rotated.token_type }}</Badge>
          <Badge variant="outline">{{ rotated.device_id }}</Badge>
          <Button variant="outline" size="sm" @click="copyToken">
            <Check v-if="copiedField === 'token'" class="size-4" />
            <Copy v-else class="size-4" />
            {{ copiedField === 'token' ? t('common.copied') : t('common.copy') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <PageSection :title="t('section.user.credentials.authTitle')" :description="t('section.user.credentials.authDescription')">
      <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(320px,420px)]">
        <form class="grid gap-4 rounded-md border p-4" @submit.prevent="startDeviceAuth">
          <div class="grid gap-2">
            <Label for="auth-device-name">{{ t('common.deviceName') }}</Label>
            <Input id="auth-device-name" v-model="authForm.device_name" required placeholder="Office PC" />
          </div>
          <div class="grid gap-2">
            <Label for="auth-public-key">Server Public Key</Label>
            <Input
              id="auth-public-key"
              v-model="authForm.server_public_key"
              required
              placeholder="base64url-public-key"
            />
          </div>
          <div class="grid gap-2 sm:grid-cols-2">
            <Button type="submit" :disabled="deviceAuthBusy || !hasDeviceAuthForm">
              <Loader2 v-if="startingAuth" class="animate-spin" />
              <Server v-else class="size-4" />
              {{ t('deviceAuth.start') }}
            </Button>
            <Button type="button" variant="outline" :disabled="deviceAuthBusy" @click="resetDeviceAuthForm">
              {{ t('common.reset') }}
            </Button>
          </div>
        </form>

        <div class="grid gap-3 rounded-md border p-4">
          <div class="flex items-center justify-between gap-3">
            <div class="flex min-w-0 items-center gap-2">
              <KeyRound class="size-4 shrink-0 text-muted-foreground" />
              <p class="font-medium">Device Code</p>
              <Badge v-if="deviceAuthStatus" variant="outline">{{ formatDeviceAuthStatus(deviceAuthStatus, t) }}</Badge>
            </div>
            <Button
              v-if="deviceAuth"
              variant="ghost"
              size="icon"
              :aria-label="t('deviceAuth.closeDeviceCode')"
              :disabled="deviceAuthBusy"
              @click="dismissDeviceAuth"
            >
              <X class="size-4" />
              <span class="sr-only">{{ t('deviceAuth.closeDeviceCode') }}</span>
            </Button>
          </div>
          <div v-if="deviceAuth" class="grid gap-3">
            <InfoRow :label="t('deviceAuth.userCode')" :value="deviceAuth.user_code" />
            <InfoRow :label="t('deviceAuth.expiresIn')" :value="formatDuration(deviceAuth.expires_in)" />
            <InfoRow :label="t('deviceAuth.pollInterval')" :value="formatDuration(deviceAuth.interval)" />
            <p class="break-all rounded-md bg-muted p-3 text-xs">{{ approvalUrl }}</p>
            <div class="grid gap-2 sm:grid-cols-2">
              <Button variant="outline" size="sm" :disabled="deviceAuthBusy" @click="copyValue(deviceAuth.user_code, 'code')">
                <Check v-if="copiedField === 'code'" class="size-4" />
                <Copy v-else class="size-4" />
                {{ copiedField === 'code' ? t('common.copied') : t('deviceAuth.copyCode') }}
              </Button>
              <Button variant="outline" size="sm" :disabled="deviceAuthBusy" @click="copyValue(approvalUrl, 'url')">
                <Check v-if="copiedField === 'url'" class="size-4" />
                <Copy v-else class="size-4" />
                {{ copiedField === 'url' ? t('common.copied') : t('deviceAuth.copyLink') }}
              </Button>
              <Button variant="outline" size="sm" :disabled="deviceAuthBusy || !approvalUrl" @click="openApprovalUrl">
                <ExternalLink class="size-4" />
                {{ t('deviceAuth.openApproval') }}
              </Button>
              <Button variant="outline" size="sm" :disabled="deviceAuthBusy" @click="approveDeviceAuth">
                <Loader2 v-if="approvingAuth" class="animate-spin" />
                <Check v-else class="size-4" />
                {{ t('deviceAuth.approveSelf') }}
              </Button>
              <Button variant="outline" size="sm" :disabled="deviceAuthBusy" @click="denyDeviceAuth">
                <Loader2 v-if="denyingAuth" class="animate-spin" />
                <KeyRound v-else class="size-4" />
                {{ t('deviceAuth.deny') }}
              </Button>
            </div>
            <Button :disabled="deviceAuthBusy" @click="pollDeviceAuth">
              <Loader2 v-if="pollingAuth" class="animate-spin" />
              <RotateCw v-else class="size-4" />
              {{ t('deviceAuth.poll') }}
            </Button>
          </div>
          <div v-else class="rounded-md bg-muted p-4 text-center text-sm text-muted-foreground">
            {{ t('deviceAuth.emptyDescription') }}
          </div>
        </div>
      </div>
    </PageSection>

    <PageSection :title="t('route.center.credentials.title')" :description="t('route.center.credentials.description')">
      <div class="grid gap-4">
        <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_220px_220px_220px_auto]">
          <SearchToolbar v-model="q" :placeholder="t('credential.searchPlaceholder')" :loading="credentials.loading.value" @refresh="credentials.refresh" />
          <Input id="credential-device-id" v-model="deviceId" :placeholder="t('common.exactDeviceId')" :aria-label="t('common.exactDeviceId')" />
          <Select :model-value="selectFilterValue(enabled)" @update:model-value="enabled = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('credential.statusFilter')"><SelectValue :placeholder="t('common.allStatus')" /></SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('common.allStatus') }}</SelectItem>
              <SelectItem value="true">{{ formatCredentialStatus(true, t) }}</SelectItem>
              <SelectItem value="false">{{ formatCredentialStatus(false, t) }}</SelectItem>
            </SelectContent>
          </Select>
          <Select v-model="sort">
            <SelectTrigger :aria-label="t('credential.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="-created_epoch_sec">{{ t('common.createdAt') }}</SelectItem>
              <SelectItem value="-last_used_epoch_sec">{{ t('common.lastUsedAt') }}</SelectItem>
              <SelectItem value="-token_version">{{ t('credential.tokenVersion') }}</SelectItem>
              <SelectItem value="device_name">{{ t('common.deviceName') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasCredentialFilters" @click="resetCredentialFilters">
            {{ t('common.reset') }}
          </Button>
        </div>
        <div class="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-center">
          <p class="text-sm text-muted-foreground sm:text-right">
            {{ t('credential.total', { total: credentials.data.value?.total ?? 0 }) }}
          </p>
        </div>
        <ResponsiveTable :items="credentials.data.value?.items ?? []" :loading="credentials.loading.value" :error="credentials.error.value" :empty-title="t('credential.empty')" @retry="credentials.refresh">
          <template #head>
            <TableRow>
              <TableHead>{{ t('credential.table.credential') }}</TableHead>
              <TableHead>{{ t('common.device') }}</TableHead>
              <TableHead>{{ t('credential.version') }}</TableHead>
              <TableHead>{{ t('credential.usage') }}</TableHead>
              <TableHead>{{ t('common.status') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="credential in credentials.data.value?.items ?? []" :key="credential.credential_id">
              <TableCell class="font-medium">{{ credential.credential_id }}</TableCell>
              <TableCell>{{ credential.device_name }}<div class="text-xs text-muted-foreground">{{ credential.device_id }}</div></TableCell>
              <TableCell>{{ credential.token_version }}</TableCell>
              <TableCell>{{ t('credential.createdPrefix', { time: formatEpoch(credential.created_epoch_sec) }) }}<div class="text-xs text-muted-foreground">{{ t('credential.lastUsedPrefix', { time: formatEpoch(credential.last_used_epoch_sec) }) }}</div></TableCell>
              <TableCell><Badge :variant="credential.enabled ? 'success' : 'secondary'">{{ formatCredentialStatus(credential.enabled, t) }}</Badge></TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Switch
                    :checked="credential.enabled"
                    :aria-label="t('credential.toggleDeviceAria', { action: credential.enabled ? t('common.disable') : t('common.enable'), device: credential.device_name })"
                    :disabled="hasBusyAction"
                    @update:checked="toggleCredential(credential.credential_id, $event)"
                  />
                  <ConfirmAction
                    :title="t('credential.rotateTitle')"
                    :description="t('credential.rotateDescription', { device: credential.device_name })"
                    :confirm-text="t('common.rotate')"
                    variant="outline"
                    :icon="RotateCw"
                    :disabled="hasBusyAction"
                    :loading="isBusy('rotate', credential.credential_id)"
                    @confirm="rotateCredential(credential.credential_id)"
                  >
                    {{ t('common.rotate') }}
                  </ConfirmAction>
                </div>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="credential in credentials.data.value?.items ?? []" :key="credential.credential_id" class="rounded-md border p-4">
              <div class="flex items-start justify-between gap-3">
                <div class="flex min-w-0 items-center gap-2">
                  <KeyRound class="size-4 text-muted-foreground" />
                  <p class="truncate font-medium">{{ credential.credential_id }}</p>
                </div>
                <Badge :variant="credential.enabled ? 'success' : 'secondary'">{{ formatCredentialStatus(credential.enabled, t) }}</Badge>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('common.device')" :value="credential.device_name" />
                <InfoRow :label="t('common.deviceId')" :value="credential.device_id" />
                <InfoRow :label="t('credential.version')" :value="credential.token_version" />
                <InfoRow :label="t('common.createdAt')" :value="formatEpoch(credential.created_epoch_sec)" />
                <InfoRow :label="t('common.lastUsedAt')" :value="formatEpoch(credential.last_used_epoch_sec)" />
              </div>
              <div class="mt-3 grid grid-cols-2 gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="hasBusyAction"
                  @click="toggleCredential(credential.credential_id, !credential.enabled)"
                >
                  <Loader2 v-if="isBusy('toggle', credential.credential_id)" class="animate-spin" />
                  {{ credential.enabled ? t('common.disable') : t('common.enable') }}
                </Button>
                <ConfirmAction
                  :title="t('credential.rotateTitle')"
                  :description="t('credential.rotateDescription', { device: credential.device_name })"
                  :confirm-text="t('common.rotate')"
                  variant="outline"
                  :icon="RotateCw"
                  :disabled="hasBusyAction"
                  :loading="isBusy('rotate', credential.credential_id)"
                  @confirm="rotateCredential(credential.credential_id)"
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
