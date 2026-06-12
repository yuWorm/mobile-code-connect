import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('admin server credentials view', () => {
  test('registers a global admin credentials route and navigation item', () => {
    const router = readFileSync(new URL('../../router/index.ts', import.meta.url), 'utf8')
    const nav = readFileSync(new URL('../../components/layout/nav.ts', import.meta.url), 'utf8')

    expect(router).toContain("path: 'credentials'")
    expect(router).toContain("name: 'admin-credentials'")
    expect(router).toContain("component: () => import('@/views/admin/AdminCredentialsView.vue')")
    expect(nav).toContain("{ label: '服务凭据', to: '/admin/credentials'")
  })

  test('lists global server credentials with exact filters and mutation actions', () => {
    const source = readFileSync(new URL('../admin/AdminCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const q = ref('')")
    expect(source).toContain("const enabled = ref('')")
    expect(source).toContain("const userId = ref('')")
    expect(source).toContain("const deviceId = ref('')")
    expect(source).toContain("const sort = ref('-created_epoch_sec')")
    expect(source).toContain('controlApi.serverCredentials(query.value)')
    expect(source).toContain('user_id: userId.value.trim()')
    expect(source).toContain('device_id: deviceId.value.trim()')
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('watch([q, enabled, userId, deviceId, sort], () => credentials.refresh())')
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'credential.searchPlaceholder\')"')
    expect(source).toContain('<Select :model-value="selectFilterValue(enabled)" @update:model-value="enabled = normalizeSelectFilterValue($event)">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'credential.statusFilter\')"><SelectValue :placeholder="t(\'common.allStatus\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem :value="ALL_SELECT_VALUE">{{ t(\'common.allStatus\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="true">{{ formatCredentialStatus(true, t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="false">{{ formatCredentialStatus(false, t) }}</SelectItem>')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'credential.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem value="-last_used_epoch_sec">{{ t(\'common.lastUsedAt\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="-token_version">{{ t(\'credential.tokenVersion\') }}</SelectItem>')
    expect(source).toContain('<Input v-model="userId"')
    expect(source).toContain('<Input v-model="deviceId"')
    expect(source).toContain('<Input v-model="userId" :placeholder="t(\'common.exactUserId\')" :aria-label="t(\'common.exactUserId\')" />')
    expect(source).toContain('<Input v-model="deviceId" :placeholder="t(\'common.exactDeviceId\')" :aria-label="t(\'common.exactDeviceId\')" />')
    expect(source).toContain("t('credential.total'")
    expect(source).toContain('controlApi.updateServerCredentialStatus')
    expect(source).toContain('controlApi.rotateServerCredential')
  })

  test('shows credential status as localized labels', () => {
    const source = readFileSync(new URL('../admin/AdminCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { formatCredentialStatus } from '@/lib/control/labels'")
    expect(source).toContain('formatCredentialStatus(credential.enabled, t)')
    expect(source).not.toContain("{{ credential.enabled ? 'enabled' : 'disabled' }}")
  })

  test('can reset global credential filters back to defaults', () => {
    const source = readFileSync(new URL('../admin/AdminCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasCredentialFilters = computed(() =>')
    expect(source).toContain("sort.value !== '-created_epoch_sec'")
    expect(source).toContain('function resetCredentialFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("enabled.value = ''")
    expect(source).toContain("userId.value = ''")
    expect(source).toContain("deviceId.value = ''")
    expect(source).toContain("sort.value = '-created_epoch_sec'")
    expect(source).toContain(':disabled="!hasCredentialFilters"')
    expect(source).toContain('@click="resetCredentialFilters"')
  })

  test('rotated server token card can be dismissed after copying', () => {
    const source = readFileSync(new URL('../admin/AdminCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { copyToClipboard } from '@/lib/control/clipboard'")
    expect(source).toContain('await copyToClipboard(rotated.value.server_token)')
    expect(source).not.toContain('navigator.clipboard.writeText')
    expect(source).toContain('function dismissRotatedToken()')
    expect(source).toContain('rotated.value = null')
    expect(source).toContain('copied.value = false')
    expect(source).toContain(':aria-label="t(\'credential.closeServerToken\')"')
    expect(source).toContain('@click="dismissRotatedToken"')
    expect(source).toContain('<X class="size-4" />')
    expect(source).toContain("<span class=\"sr-only\">{{ t('credential.closeServerToken') }}</span>")
  })

  test('credential status switch has a descriptive accessible label', () => {
    const source = readFileSync(new URL('../admin/AdminCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain(
      ":aria-label=\"t('credential.toggleAria', { action: credential.enabled ? t('common.disable') : t('common.enable'), user: credential.user_id, device: credential.device_name })\"",
    )
    expect(source).toContain(':checked="credential.enabled"')
    expect(source).toContain('@update:checked="toggleCredential(credential.credential_id, $event)"')
  })

  test('global credentials visible copy is localized', () => {
    const source = readFileSync(new URL('../admin/AdminCredentialsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("success: nextEnabled ? t('credential.toast.enabled') : t('credential.toast.disabled')")
    expect(source).toContain("error: t('credential.toast.statusFailed')")
    expect(source).toContain("success: t('credential.toast.rotated')")
    expect(source).toContain("error: t('credential.toast.rotateFailed')")
    expect(source).toContain("<CardTitle>{{ t('credential.newServerToken') }}</CardTitle>")
    expect(source).toContain("{{ copied ? t('common.copied') : t('common.copy') }}")
    expect(source).toContain("{{ t('common.reset') }}")
    expect(source).toContain(":empty-title=\"t('credential.empty')\"")
    expect(source).toContain("<TableHead>{{ t('credential.table.credential') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.user') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.device') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('credential.version') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('credential.usage') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.status') }}</TableHead>")
    expect(source).toContain("<TableHead class=\"text-right\">{{ t('common.actions') }}</TableHead>")
    expect(source).toContain("{{ t('credential.createdPrefix', { time: formatEpoch(credential.created_epoch_sec) }) }}")
    expect(source).toContain("{{ t('credential.lastUsedPrefix', { time: formatEpoch(credential.last_used_epoch_sec) }) }}")
    expect(source).toContain(":title=\"t('credential.rotateTitle')\"")
    expect(source).toContain(":description=\"t('credential.rotateDescription', { device: credential.device_name })\"")
    expect(source).toContain(":confirm-text=\"t('common.rotate')\"")
    expect(source).toContain(":label=\"t('credential.version')\"")
    expect(source).toContain(":label=\"t('common.createdAt')\"")
    expect(source).toContain(":label=\"t('common.lastUsedAt')\"")
    expect(source).not.toContain('服务凭据已启用')
    expect(source).not.toContain('新的 Server Token')
    expect(source).not.toContain('暂无服务凭据')
    expect(source).not.toContain('轮换服务凭据')
  })
})
