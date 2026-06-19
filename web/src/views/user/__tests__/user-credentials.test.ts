import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('UserCredentialsView', () => {
  test('exposes device-code server auth actions', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('startDeviceServerAuth')
    expect(source).toContain('approveDeviceServerAuth')
    expect(source).toContain('denyDeviceServerAuth')
    expect(source).toContain('pollDeviceServerAuth')
    expect(source).toContain('verification_uri_complete')
    expect(source).toContain('async function denyDeviceAuth()')
    expect(source).toContain('const denial = await controlApi.denyDeviceServerAuth(deviceAuth.value!.user_code)')
    expect(source).toContain("success: t('deviceAuth.toast.denied')")
    expect(source).toContain('@click="denyDeviceAuth"')
  })

  test('device-code auth form can be cleared before starting a new attempt', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasDeviceAuthForm = computed(() =>')
    expect(source).toContain("authForm.device_name.trim() !== ''")
    expect(source).toContain("authForm.server_public_key.trim() !== ''")
    expect(source).toContain('function resetDeviceAuthForm()')
    expect(source).toContain("authForm.device_name = ''")
    expect(source).toContain("authForm.server_public_key = ''")
    expect(source).toContain('deviceAuth.value = null')
    expect(source).toContain("deviceAuthStatus.value = ''")
    expect(source).toContain(':disabled="deviceAuthBusy"')
    expect(source).toContain(':disabled="deviceAuthBusy || !hasDeviceAuthForm"')
    expect(source).toContain('@click="resetDeviceAuthForm"')
    expect(source).not.toContain('authForm.device_id')
    expect(source).not.toContain('id="auth-device-id"')
    expect(source).not.toContain("device_id: authForm.device_id.trim()")
  })

  test('device-code auth start omits device id so Control Server generates it', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain(`deviceAuth.value = await controlApi.startDeviceServerAuth({
          device_name: authForm.device_name.trim(),
          server_public_key: authForm.server_public_key.trim(),
        })`)
    expect(source).not.toContain("device_id: authForm.device_id.trim()")
  })

  test('polling displays the generated device id from the issued credential', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('rotated.value = poll.credential')
    expect(source).toContain('<Badge variant="outline">{{ rotated.device_id }}</Badge>')
  })

  test('device-code auth actions share a busy guard to avoid duplicate submissions', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const approvingAuth = ref(false)')
    expect(source).toContain('const denyingAuth = ref(false)')
    expect(source).toContain('const deviceAuthBusy = computed(() =>')
    expect(source).toContain('startingAuth.value || pollingAuth.value || approvingAuth.value || denyingAuth.value')
    expect(source).toContain('if (!deviceAuth.value || deviceAuthBusy.value) {')
    expect(source).toContain('approvingAuth.value = true')
    expect(source).toContain('approvingAuth.value = false')
    expect(source).toContain('denyingAuth.value = true')
    expect(source).toContain('denyingAuth.value = false')
    expect(source).toContain('pollingAuth.value = true')
    expect(source).toContain(':disabled="deviceAuthBusy || !approvalUrl"')
    expect(source).toContain('<Loader2 v-if="approvingAuth" class="animate-spin" />')
    expect(source).toContain('<Loader2 v-if="denyingAuth" class="animate-spin" />')
    expect(source).toContain('<Loader2 v-if="pollingAuth" class="animate-spin" />')
  })

  test('filters server credentials by search and enabled state', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const q = ref('')")
    expect(source).toContain("const deviceId = ref('')")
    expect(source).toContain("const enabled = ref('')")
    expect(source).toContain("const sort = ref('-created_epoch_sec')")
    expect(source).toContain('const credentialsQuery = computed(() => ({')
    expect(source).toContain('q: q.value.trim()')
    expect(source).toContain('device_id: deviceId.value.trim()')
    expect(source).toContain("enabled: enabled.value === '' ? undefined : enabled.value === 'true'")
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('watch([q, deviceId, enabled, sort], () => credentials.refresh())')
    expect(source).toContain('<SearchToolbar v-model="q"')
    expect(source).toContain('<Input id="credential-device-id" v-model="deviceId" :placeholder="t(\'common.exactDeviceId\')" :aria-label="t(\'common.exactDeviceId\')" />')
    expect(source).toContain('<Select :model-value="selectFilterValue(enabled)" @update:model-value="enabled = normalizeSelectFilterValue($event)">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'credential.statusFilter\')"><SelectValue :placeholder="t(\'common.allStatus\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem :value="ALL_SELECT_VALUE">{{ t(\'common.allStatus\') }}</SelectItem>')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'credential.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem value="true">{{ formatCredentialStatus(true, t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="false">{{ formatCredentialStatus(false, t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="-last_used_epoch_sec">{{ t(\'common.lastUsedAt\') }}</SelectItem>')
    expect(source).toContain('credentials.data.value?.total')
  })

  test('shows device-code and credential statuses as localized labels', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { formatDuration, formatEpoch } from '@/lib/control/format'")
    expect(source).toContain("import { formatCredentialStatus, formatDeviceAuthStatus } from '@/lib/control/labels'")
    expect(source).toContain('formatDeviceAuthStatus(deviceAuthStatus, t)')
    expect(source).toContain('formatDuration(deviceAuth.expires_in)')
    expect(source).toContain('formatDuration(deviceAuth.interval)')
    expect(source).toContain('formatDuration(poll.interval)')
    expect(source).toContain('formatCredentialStatus(credential.enabled, t)')
    expect(source).toContain('<InfoRow :label="t(\'deviceAuth.expiresIn\')" :value="formatDuration(deviceAuth.expires_in)" />')
    expect(source).not.toContain('过期秒数')
    expect(source).not.toContain("{{ credential.enabled ? 'enabled' : 'disabled' }}")
  })

  test('can reset server credential filters back to defaults', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasCredentialFilters = computed(() =>')
    expect(source).toContain("sort.value !== '-created_epoch_sec'")
    expect(source).toContain('function resetCredentialFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("deviceId.value = ''")
    expect(source).toContain("enabled.value = ''")
    expect(source).toContain("sort.value = '-created_epoch_sec'")
    expect(source).toContain(':disabled="!hasCredentialFilters"')
    expect(source).toContain('@click="resetCredentialFilters"')
  })

  test('issued or rotated server token card can be dismissed after copying', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { copyToClipboard } from '@/lib/control/clipboard'")
    expect(source).toContain('await copyToClipboard(value)')
    expect(source).not.toContain('navigator.clipboard.writeText')
    expect(source).toContain('function dismissRotatedToken()')
    expect(source).toContain('rotated.value = null')
    expect(source).toContain('copiedField.value = null')
    expect(source).toContain(':aria-label="t(\'credential.closeServerToken\')"')
    expect(source).toContain('@click="dismissRotatedToken"')
    expect(source).toContain('<X class="size-4" />')
    expect(source).toContain('<span class="sr-only">{{ t(\'credential.closeServerToken\') }}</span>')
  })

  test('device-code auth result can be dismissed after copying', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function dismissDeviceAuth()')
    expect(source).toContain('deviceAuth.value = null')
    expect(source).toContain("deviceAuthStatus.value = ''")
    expect(source).toContain('copiedField.value = null')
    expect(source).toContain(':aria-label="t(\'deviceAuth.closeDeviceCode\')"')
    expect(source).toContain('@click="dismissDeviceAuth"')
    expect(source).toContain('<span class="sr-only">{{ t(\'deviceAuth.closeDeviceCode\') }}</span>')
  })

  test('starting device-code auth clears stale results before loading', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain(`async function startDeviceAuth() {
  if (!hasDeviceAuthForm.value || deviceAuthBusy.value) {
    return
  }
  startingAuth.value = true
  deviceAuth.value = null
  deviceAuthStatus.value = ''
  copiedField.value = null`)
  })

  test('credential status switch has a descriptive accessible label', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain(":aria-label=\"t('credential.toggleDeviceAria', { action: credential.enabled ? t('common.disable') : t('common.enable'), device: credential.device_name })\"")
    expect(source).toContain(':checked="credential.enabled"')
    expect(source).toContain('@update:checked="toggleCredential(credential.credential_id, $event)"')
  })

  test('device-code auth and credential list visible copy is localized', () => {
    const source = readFileSync(new URL('../UserCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("success: t('deviceAuth.toast.started')")
    expect(source).toContain("error: t('deviceAuth.toast.startFailed')")
    expect(source).toContain("success: t('deviceAuth.toast.approved')")
    expect(source).toContain("error: t('deviceAuth.toast.approveFailed')")
    expect(source).toContain("error: t('deviceAuth.toast.denyFailed')")
    expect(source).toContain("toast(t('deviceAuth.toast.pending')")
    expect(source).toContain("description: t('deviceAuth.toast.pendingDescription', { duration: formatDuration(poll.interval) })")
    expect(source).toContain("success: (result) => (result === 'issued' ? t('deviceAuth.toast.issued') : '')")
    expect(source).toContain("error: t('deviceAuth.toast.pollFailed')")
    expect(source).toContain('<CardTitle>{{ t(\'credential.newServerToken\') }}</CardTitle>')
    expect(source).toContain("{{ copiedField === 'token' ? t('common.copied') : t('common.copy') }}")
    expect(source).toContain('<Label for="auth-device-name">{{ t(\'common.deviceName\') }}</Label>')
    expect(source).toContain("{{ t('deviceAuth.start') }}")
    expect(source).toContain("{{ t('common.reset') }}")
    expect(source).toContain("{{ copiedField === 'code' ? t('common.copied') : t('deviceAuth.copyCode') }}")
    expect(source).toContain("{{ copiedField === 'url' ? t('common.copied') : t('deviceAuth.copyLink') }}")
    expect(source).toContain("{{ t('deviceAuth.openApproval') }}")
    expect(source).toContain("{{ t('deviceAuth.approveSelf') }}")
    expect(source).toContain("{{ t('deviceAuth.deny') }}")
    expect(source).toContain("{{ t('deviceAuth.poll') }}")
    expect(source).toContain("{{ t('deviceAuth.emptyDescription') }}")
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'credential.searchPlaceholder\')"')
    expect(source).toContain("{{ t('credential.total', { total: credentials.data.value?.total ?? 0 }) }}")
    expect(source).toContain(':empty-title="t(\'credential.empty\')"')
    expect(source).toContain('<TableHead>{{ t(\'credential.table.credential\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'common.device\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'credential.version\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'credential.usage\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'common.status\') }}</TableHead>')
    expect(source).toContain('<TableHead class="text-right">{{ t(\'common.actions\') }}</TableHead>')
    expect(source).toContain("t('credential.createdPrefix', { time: formatEpoch(credential.created_epoch_sec) })")
    expect(source).toContain("t('credential.lastUsedPrefix', { time: formatEpoch(credential.last_used_epoch_sec) })")
    expect(source).toContain(':title="t(\'credential.rotateTitle\')"')
    expect(source).toContain(":description=\"t('credential.rotateDescription', { device: credential.device_name })\"")
    expect(source).not.toContain('发起认证')
    expect(source).not.toContain('服务凭据已启用')
    expect(source).not.toContain('暂无服务凭据')
    expect(source).not.toContain('轮换服务凭据')
  })
})
