<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Check, Copy, KeyRound, Loader2, RotateCw, Server, X } from 'lucide-vue-next'

import ConfirmAction from '@/components/layout/ConfirmAction.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { copyToClipboard } from '@/lib/control/clipboard'
import { formatEpoch } from '@/lib/control/format'
import { formatCredentialStatus } from '@/lib/control/labels'
import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '@/lib/control/select-filter'
import type { ServerCredentialResponse } from '@/lib/control/types'

const q = ref('')
const enabled = ref('')
const userId = ref('')
const deviceId = ref('')
const sort = ref('-created_epoch_sec')
const rotated = ref<ServerCredentialResponse | null>(null)
const copied = ref(false)
const { t } = useI18n()
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
const query = computed(() => ({
  q: q.value.trim(),
  enabled: enabled.value === '' ? undefined : enabled.value === 'true',
  user_id: userId.value.trim(),
  device_id: deviceId.value.trim(),
  limit: 100,
  sort: sort.value,
}))
const hasCredentialFilters = computed(() =>
  q.value.trim() !== '' ||
  enabled.value !== '' ||
  userId.value.trim() !== '' ||
  deviceId.value.trim() !== '' ||
  sort.value !== '-created_epoch_sec',
)
const credentials = useAsyncData(() => controlApi.serverCredentials(query.value))
watch([q, enabled, userId, deviceId, sort], () => credentials.refresh())

function resetCredentialFilters() {
  q.value = ''
  enabled.value = ''
  userId.value = ''
  deviceId.value = ''
  sort.value = '-created_epoch_sec'
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
        copied.value = false
        await credentials.refresh()
      },
      {
        success: t('credential.toast.rotated'),
        error: t('credential.toast.rotateFailed'),
      },
    )
  })
}

async function copyToken() {
  if (!rotated.value) {
    return
  }
  await copyToClipboard(rotated.value.server_token)
  copied.value = true
  window.setTimeout(() => {
    copied.value = false
  }, 1600)
}

function dismissRotatedToken() {
  rotated.value = null
  copied.value = false
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
            <Check v-if="copied" class="size-4" />
            <Copy v-else class="size-4" />
            {{ copied ? t('common.copied') : t('common.copy') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <PageSection :title="t('route.admin.credentials.title')" :description="t('route.admin.credentials.description')">
      <div class="grid gap-4">
        <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_220px_220px_auto]">
          <SearchToolbar v-model="q" :placeholder="t('credential.searchPlaceholder')" :loading="credentials.loading.value" @refresh="credentials.refresh" />
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
              <SelectItem value="credential_id">{{ t('common.credentialId') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasCredentialFilters" @click="resetCredentialFilters">
            {{ t('common.reset') }}
          </Button>
        </div>
        <div class="grid gap-3 md:grid-cols-[1fr_1fr_auto] md:items-center">
          <Input v-model="userId" :placeholder="t('common.exactUserId')" :aria-label="t('common.exactUserId')" />
          <Input v-model="deviceId" :placeholder="t('common.exactDeviceId')" :aria-label="t('common.exactDeviceId')" />
          <p class="text-sm text-muted-foreground md:text-right">
            {{ t('credential.total', { total: credentials.data.value?.total ?? 0 }) }}
          </p>
        </div>

        <ResponsiveTable
          :items="credentials.data.value?.items ?? []"
          :loading="credentials.loading.value"
          :error="credentials.error.value"
          :empty-title="t('credential.empty')"
          @retry="credentials.refresh"
        >
          <template #head>
            <TableRow>
              <TableHead>{{ t('credential.table.credential') }}</TableHead>
              <TableHead>{{ t('common.user') }}</TableHead>
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
              <TableCell>{{ credential.user_id }}</TableCell>
              <TableCell>
                {{ credential.device_name }}
                <div class="text-xs text-muted-foreground">{{ credential.device_id }}</div>
              </TableCell>
              <TableCell>{{ credential.token_version }}</TableCell>
              <TableCell>
                {{ t('credential.createdPrefix', { time: formatEpoch(credential.created_epoch_sec) }) }}
                <div class="text-xs text-muted-foreground">{{ t('credential.lastUsedPrefix', { time: formatEpoch(credential.last_used_epoch_sec) }) }}</div>
              </TableCell>
              <TableCell>
                <Badge :variant="credential.enabled ? 'success' : 'secondary'">
                  {{ formatCredentialStatus(credential.enabled, t) }}
                </Badge>
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Switch
                    :checked="credential.enabled"
                    :aria-label="t('credential.toggleAria', { action: credential.enabled ? t('common.disable') : t('common.enable'), user: credential.user_id, device: credential.device_name })"
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
                  <Server class="size-4 text-muted-foreground" />
                  <p class="truncate font-medium">{{ credential.credential_id }}</p>
                </div>
                <Badge :variant="credential.enabled ? 'success' : 'secondary'">
                  {{ formatCredentialStatus(credential.enabled, t) }}
                </Badge>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('common.user')" :value="credential.user_id" />
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
                  <KeyRound v-else class="size-4" />
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
