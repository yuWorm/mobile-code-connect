<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { Check, KeyRound, Loader2, X } from 'lucide-vue-next'

import InfoRow from '@/components/layout/InfoRow.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useI18n } from '@/composables/useI18n'
import { controlApiErrorMessage } from '@/lib/control/api'
import { controlApi } from '@/lib/control/client'
import { formatEpoch } from '@/lib/control/format'
import { formatDeviceAuthStatus } from '@/lib/control/labels'
import type { ServerAuthSessionDetail } from '@/lib/control/types'

const route = useRoute()
const { t } = useI18n()
const userCodeInput = ref(String(route.query.user_code ?? ''))
const detail = ref<ServerAuthSessionDetail | null>(null)
const loading = ref(false)
const approving = ref(false)
const denying = ref(false)
const error = ref('')
const normalizedUserCode = computed(() => userCodeInput.value.trim())
const canLoad = computed(() => normalizedUserCode.value !== '' && !loading.value)
const canDecide = computed(() =>
  Boolean(detail.value && detail.value.status === 'pending' && !approving.value && !denying.value),
)

onMounted(() => {
  if (normalizedUserCode.value) {
    void loadDetail()
  }
})

async function loadDetail() {
  if (!normalizedUserCode.value) {
    error.value = t('serverAuthApproval.deviceMissingUserCode')
    return
  }
  loading.value = true
  error.value = ''
  try {
    detail.value = await controlApi.deviceServerAuthSessionDetail(normalizedUserCode.value)
  } catch (caught) {
    detail.value = null
    error.value = controlApiErrorMessage(caught, {
      unauthorized: t('serverAuthApproval.loginRequired'),
      fallback: t('serverAuthApproval.loadFailed'),
    })
  } finally {
    loading.value = false
  }
}

async function approve() {
  if (!canDecide.value) {
    return
  }
  approving.value = true
  error.value = ''
  try {
    const response = await controlApi.approveDeviceServerAuth(normalizedUserCode.value)
    if (detail.value) {
      detail.value = { ...detail.value, status: response.status }
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

async function deny() {
  if (!canDecide.value) {
    return
  }
  denying.value = true
  error.value = ''
  try {
    const response = await controlApi.denyDeviceServerAuth(normalizedUserCode.value)
    if (detail.value) {
      detail.value = { ...detail.value, status: response.status }
    }
  } catch (caught) {
    error.value = controlApiErrorMessage(caught, {
      unauthorized: t('serverAuthApproval.loginRequired'),
      fallback: t('serverAuthApproval.denyFailed'),
    })
  } finally {
    denying.value = false
  }
}
</script>

<template>
  <main class="flex min-h-screen items-center justify-center bg-background px-4 py-8">
    <Card class="w-full max-w-2xl">
      <CardHeader>
        <div class="mb-2 flex size-10 items-center justify-center rounded-md bg-primary text-primary-foreground">
          <KeyRound class="size-5" />
        </div>
        <CardTitle>{{ t('serverAuthApproval.deviceTitle') }}</CardTitle>
        <CardDescription>{{ t('serverAuthApproval.deviceDescription') }}</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-5">
        <form class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]" @submit.prevent="loadDetail">
          <div class="grid gap-2">
            <Label for="server-auth-user-code">{{ t('serverAuthApproval.userCode') }}</Label>
            <Input id="server-auth-user-code" v-model="userCodeInput" autocomplete="one-time-code" :placeholder="t('serverAuthApproval.userCodePlaceholder')" />
          </div>
          <Button class="self-end" type="submit" :disabled="!canLoad">
            <Loader2 v-if="loading" class="animate-spin" />
            <KeyRound v-else class="size-4" />
            {{ t('serverAuthApproval.loadDeviceSession') }}
          </Button>
        </form>

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
            <InfoRow :label="t('serverAuthApproval.userCode')" :value="normalizedUserCode" />
            <InfoRow :label="t('common.deviceName')" :value="detail.device_name" />
            <InfoRow :label="t('common.deviceId')" :value="detail.device_id" />
            <InfoRow :label="t('serverAuthApproval.publicKeyFingerprint')" :value="detail.server_public_key_fingerprint" />
            <InfoRow :label="t('common.expiresAt')" :value="formatEpoch(detail.expires_epoch_sec)" />
          </div>

          <div class="flex flex-wrap gap-3">
            <Button :disabled="!canDecide" @click="approve">
              <Loader2 v-if="approving" class="animate-spin" />
              <Check v-else class="size-4" />
              {{ t('serverAuthApproval.approve') }}
            </Button>
            <Button variant="outline" :disabled="!canDecide" @click="deny">
              <Loader2 v-if="denying" class="animate-spin" />
              <X v-else class="size-4" />
              {{ t('serverAuthApproval.deny') }}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  </main>
</template>
