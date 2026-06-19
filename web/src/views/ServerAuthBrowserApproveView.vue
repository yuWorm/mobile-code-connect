<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { Check, Copy, Loader2, ShieldCheck } from 'lucide-vue-next'

import InfoRow from '@/components/layout/InfoRow.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { useI18n } from '@/composables/useI18n'
import { controlApiErrorMessage } from '@/lib/control/api'
import { controlApi } from '@/lib/control/client'
import { copyToClipboard } from '@/lib/control/clipboard'
import { formatEpoch } from '@/lib/control/format'
import { formatDeviceAuthStatus } from '@/lib/control/labels'
import type { BrowserServerAuthApprovalResponse, ServerAuthSessionDetail } from '@/lib/control/types'

const route = useRoute()
const { t } = useI18n()
const sessionId = String(route.query.session_id ?? '')
const detail = ref<ServerAuthSessionDetail | null>(null)
const approval = ref<BrowserServerAuthApprovalResponse | null>(null)
const loading = ref(false)
const approving = ref(false)
const copied = ref(false)
const error = ref('')
const authCode = computed(() => approval.value?.server_auth_code ?? '')
const canApprove = computed(() =>
  Boolean(detail.value && !approval.value && detail.value.status === 'pending' && !approving.value),
)

onMounted(loadDetail)

async function loadDetail() {
  if (!sessionId) {
    error.value = t('serverAuthApproval.browserMissingSession')
    return
  }
  loading.value = true
  error.value = ''
  try {
    detail.value = await controlApi.browserServerAuthSessionDetail(sessionId)
  } catch (caught) {
    error.value = controlApiErrorMessage(caught, {
      unauthorized: t('serverAuthApproval.loginRequired'),
      fallback: t('serverAuthApproval.loadFailed'),
    })
  } finally {
    loading.value = false
  }
}

async function approve() {
  if (!canApprove.value) {
    return
  }
  approving.value = true
  error.value = ''
  try {
    approval.value = await controlApi.approveBrowserServerAuth(sessionId)
    if (detail.value) {
      detail.value = { ...detail.value, status: approval.value.status }
    }
  } catch (caught) {
    error.value = controlApiErrorMessage(caught, {
      unauthorized: t('serverAuthApproval.loginRequired'),
      fallback: t('serverAuthApproval.approveFailed'),
    })
  } finally {
    approving.value = false
  }
}

async function copyAuthCode() {
  if (!approval.value?.server_auth_code) {
    return
  }
  await copyToClipboard(approval.value.server_auth_code)
  copied.value = true
  window.setTimeout(() => {
    copied.value = false
  }, 1600)
}
</script>

<template>
  <main class="flex min-h-screen items-center justify-center bg-background px-4 py-8">
    <Card class="w-full max-w-2xl">
      <CardHeader>
        <div class="mb-2 flex size-10 items-center justify-center rounded-md bg-primary text-primary-foreground">
          <ShieldCheck class="size-5" />
        </div>
        <CardTitle>{{ t('serverAuthApproval.browserTitle') }}</CardTitle>
        <CardDescription>{{ t('serverAuthApproval.browserDescription') }}</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-5">
        <div v-if="loading" class="flex items-center gap-2 rounded-md border p-3 text-sm text-muted-foreground">
          <Loader2 class="size-4 animate-spin" />
          {{ t('serverAuthApproval.loading') }}
        </div>

        <p v-if="error" class="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          {{ error }}
        </p>

        <div v-if="detail" class="grid gap-4">
          <div class="flex flex-wrap items-center gap-2">
            <Badge variant="outline">{{ formatDeviceAuthStatus(detail.status, t) }}</Badge>
            <Badge variant="outline">{{ detail.mode }}</Badge>
          </div>
          <div class="grid gap-1 rounded-md border p-4">
            <InfoRow :label="t('common.sessionId')" :value="detail.session_id" />
            <InfoRow :label="t('common.deviceName')" :value="detail.device_name" />
            <InfoRow :label="t('common.deviceId')" :value="detail.device_id" />
            <InfoRow :label="t('serverAuthApproval.publicKeyFingerprint')" :value="detail.server_public_key_fingerprint" />
            <InfoRow :label="t('common.expiresAt')" :value="formatEpoch(detail.expires_epoch_sec)" />
          </div>

          <div v-if="authCode" class="grid gap-3 rounded-md border border-primary/30 bg-primary/5 p-4">
            <p class="text-sm font-medium">{{ t('serverAuthApproval.authCode') }}</p>
            <p class="break-all rounded-md bg-background p-3 font-mono text-sm">{{ authCode }}</p>
            <Button variant="outline" class="w-fit" @click="copyAuthCode">
              <Check v-if="copied" class="size-4" />
              <Copy v-else class="size-4" />
              {{ copied ? t('common.copied') : t('serverAuthApproval.copyAuthCode') }}
            </Button>
          </div>

          <div class="flex flex-wrap gap-3">
            <Button :disabled="!canApprove" @click="approve">
              <Loader2 v-if="approving" class="animate-spin" />
              <ShieldCheck v-else class="size-4" />
              {{ t('serverAuthApproval.approve') }}
            </Button>
            <Button variant="outline" :disabled="loading || approving" @click="loadDetail">
              {{ t('common.retry') }}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  </main>
</template>
