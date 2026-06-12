import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

import { messages } from '@/lib/i18n/messages'

describe('dashboard refresh controls', () => {
  test('admin dashboard exposes a manual refresh action while preserving initial loading states', () => {
    const source = readFileSync(new URL('../admin/AdminDashboardView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(source).toContain('const { t } = useI18n()')
    expect(source).toContain("import { Button } from '@/components/ui/button'")
    expect(source).toContain('Loader2')
    expect(source).toContain('RefreshCw')
    expect(source).toContain('const { data, loading, error, refresh } = useAsyncData(() => controlApi.dashboard())')
    expect(source).toContain(':disabled="loading"')
    expect(source).toContain('@click="refresh"')
    expect(source).toContain('<Loader2 v-if="loading" class="animate-spin" />')
    expect(source).toContain('<RefreshCw v-else class="size-4" />')
    expect(source).toContain('<LoadingState v-if="loading && !data" :label="t(\'dashboard.admin.loading\')" />')
    expect(source).toContain('<ErrorState v-else-if="error && !data" :message="error" @retry="refresh" />')
  })

  test('admin dashboard summary cards use localized status labels', () => {
    const source = readFileSync(new URL('../admin/AdminDashboardView.vue', import.meta.url), 'utf8')

    expect(source).toContain("t('dashboard.admin.sessionsDescription'")
    expect(source).toContain("t('dashboard.admin.relaysDescription'")
    expect(source).toContain("import { formatAuditTargetType } from '@/lib/control/labels'")
    expect(source).toContain('formatAuditTargetType(log.target_type, t)')
    expect(source).not.toContain('pending，')
    expect(source).not.toContain('bound`')
    expect(source).not.toContain('healthy，')
    expect(source).not.toContain('unhealthy`')
  })

  test('user dashboard can refresh all panels and report aggregate request errors', () => {
    const source = readFileSync(new URL('../user/UserDashboardView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { Button } from '@/components/ui/button'")
    expect(source).toContain('const dashboardLoading = computed(() =>')
    expect(source).toContain('plan.loading.value ||')
    expect(source).toContain('devices.loading.value ||')
    expect(source).toContain('controllers.loading.value ||')
    expect(source).toContain('credentials.loading.value')
    expect(source).toContain('const dashboardError = computed(() =>')
    expect(source).toContain('plan.error.value ||')
    expect(source).toContain('devices.error.value ||')
    expect(source).toContain('controllers.error.value ||')
    expect(source).toContain('credentials.error.value')
    expect(source).toContain('function refreshDashboard()')
    expect(source).toContain('return Promise.all([')
    expect(source).toContain('plan.refresh(),')
    expect(source).toContain('devices.refresh(),')
    expect(source).toContain('controllers.refresh(),')
    expect(source).toContain('credentials.refresh(),')
    expect(source).toContain(':disabled="dashboardLoading"')
    expect(source).toContain('@click="refreshDashboard"')
    expect(source).toContain(':message="dashboardError"')
    expect(source).toContain('@retry="refreshDashboard"')
    expect(source).toContain("import { formatRoleLabel } from '@/lib/control/labels'")
    expect(source).toContain("formatRoleLabel(state.session?.role, t)")
    expect(source).not.toContain('t(formatRoleLabel')
  })

  test('user dashboard uses the same plan limit labels as admin plan pages', () => {
    const source = readFileSync(new URL('../user/UserDashboardView.vue', import.meta.url), 'utf8')

    expect(source).toContain("t('dashboard.user.bandwidthLimit')")
    expect(source).toContain("t('dashboard.user.bandwidthMeta'")
    expect(source).toContain('streams: plan.data.value.relay_limits.max_streams')
    expect(source).toContain('duration: formatDuration(plan.data.value.relay_limits.max_duration_sec)')
    expect(source).not.toContain('Relay 限速')
    expect(source).not.toContain('{{ plan.data.value.relay_limits.max_streams }} streams')
  })

  test('user dashboard visible copy is fully localized', () => {
    const source = readFileSync(new URL('../user/UserDashboardView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const shortcuts = computed(() =>')
    expect(source).toContain("title: t('dashboard.user.shortcut.devicesTitle')")
    expect(source).toContain("description: t('dashboard.user.shortcut.devicesDescription')")
    expect(source).toContain("{{ t('route.center.dashboard.title') }}")
    expect(source).toContain("{{ t('route.center.dashboard.description') }}")
    expect(source).toContain("{{ t('dashboard.user.refresh') }}")
    expect(source).toContain(":label=\"t('dashboard.user.accessibleDevices')\"")
    expect(source).toContain(":description=\"t('dashboard.user.accessibleDevicesDescription'")
    expect(source).toContain(":label=\"t('dashboard.user.currentRole')\"")
    expect(source).toContain('<LoadingState v-if="plan.loading.value && !plan.data.value" :label="t(\'dashboard.user.planLoading\')" />')
    expect(source).toContain("t('dashboard.user.maxControllers')")
    expect(source).toContain("t('dashboard.user.trafficQuota')")
    expect(source).not.toContain('用户中台')
    expect(source).not.toContain('设备访问、控制器')
    expect(source).not.toContain('可访问设备')
    expect(source).not.toContain('客户端身份数量')
    expect(source).not.toContain('受控服务器登录凭据')
    expect(source).not.toContain('当前角色')
    expect(source).not.toContain('加载套餐')
    expect(source).not.toContain('最大控制器')
    expect(source).not.toContain('流量配额')

    expect(messages['zh-CN']['dashboard.user.shortcut.devicesTitle']).toBe('我的设备')
    expect(messages['en-US']['dashboard.user.shortcut.devicesTitle']).toBe('My Devices')
    expect(messages['zh-CN']['section.user.dashboard.shortcutsTitle']).toBe('常用入口')
    expect(messages['en-US']['section.user.dashboard.shortcutsTitle']).toBe('Shortcuts')
  })
})
