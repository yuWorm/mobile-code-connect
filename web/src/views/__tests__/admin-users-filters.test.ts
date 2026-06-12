import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('admin user filters', () => {
  test('users view filters by role and enabled state', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const role = ref('')")
    expect(source).toContain("const enabled = ref('')")
    expect(source).toContain("const sort = ref('email')")
    expect(source).toContain('role: role.value')
    expect(source).toContain("enabled: enabled.value === '' ? undefined : enabled.value === 'true'")
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('watch([q, role, enabled, sort], () => users.refresh())')
    expect(source).toContain('<Select :model-value="selectFilterValue(role)" @update:model-value="role = normalizeSelectFilterValue($event)">')
    expect(source).toContain('<Select :model-value="selectFilterValue(enabled)" @update:model-value="enabled = normalizeSelectFilterValue($event)">')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'adminUser.roleFilter\')">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'adminUser.accountStatusFilter\')">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'adminUser.sortLabel\')">')
    expect(source).toContain('<SelectItem :value="ALL_SELECT_VALUE">{{ t(\'adminUser.allRoles\') }}</SelectItem>')
    expect(source).toContain('<SelectItem :value="ALL_SELECT_VALUE">{{ t(\'common.allStatus\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="user">{{ formatRoleLabel(\'user\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="admin">{{ formatRoleLabel(\'admin\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="true">{{ formatEnabledLabel(true, t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="false">{{ formatEnabledLabel(false, t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="controller_count">{{ t(\'adminUser.sortControllers\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="device_count">{{ t(\'adminUser.sortDevices\') }}</SelectItem>')
    expect(source).toContain('users.data.value?.total')
    expect(source).not.toContain('(usage.data.value?.items.length ?? 0) === 0')
  })

  test('users view shows localized role, account, and detail device labels', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { formatDeviceStatus, formatEnabledLabel, formatRoleLabel } from '@/lib/control/labels'")
    expect(source).toContain('formatEnabledLabel(user.enabled, t)')
    expect(source).toContain('formatRoleLabel(user.role, t)')
    expect(source).toContain('formatEnabledLabel(selectedUser.user.enabled, t)')
    expect(source).toContain('formatDeviceStatus(device.status, t)')
    expect(source).not.toContain("{{ user.enabled ? 'enabled' : 'disabled' }}")
    expect(source).not.toContain("selectedUser.user.enabled ? 'enabled' : 'disabled'")
    expect(source).not.toContain('{{ device.status }}')
  })

  test('users list filters can be reset to defaults', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasUserFilters = computed(() =>')
    expect(source).toContain("q.value.trim() !== ''")
    expect(source).toContain("role.value !== ''")
    expect(source).toContain("enabled.value !== ''")
    expect(source).toContain("sort.value !== 'email'")
    expect(source).toContain('function resetUserFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("role.value = ''")
    expect(source).toContain("enabled.value = ''")
    expect(source).toContain("sort.value = 'email'")
    expect(source).toContain(':disabled="!hasUserFilters"')
    expect(source).toContain('@click="resetUserFilters"')
  })

  test('new user dialog resets stale form values before opening', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function resetCreateForm()')
    expect(source).toContain("createForm.email = ''")
    expect(source).toContain("createForm.password = ''")
    expect(source).toContain("createForm.display_name = ''")
    expect(source).toContain("createForm.role = 'user'")
    expect(source).toContain('createForm.enabled = true')
    expect(source).toContain('@click="resetCreateForm"')
  })

  test('new user dialog exposes labelled role and status controls plus inline reset', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasCreateForm = computed(() =>')
    expect(source).toContain("createForm.email.trim() !== ''")
    expect(source).toContain("createForm.password.trim() !== ''")
    expect(source).toContain("createForm.display_name.trim() !== ''")
    expect(source).toContain('autocomplete="email"')
    expect(source).toContain('autocomplete="name"')
    expect(source).toContain('autocomplete="new-password"')
    expect(source).toContain('for="new-role"')
    expect(source).toContain('id="new-role"')
    expect(source).toContain('for="new-enabled"')
    expect(source).toContain('id="new-enabled"')
    expect(source).toContain('type="button"')
    expect(source).toContain(':disabled="creating"')
    expect(source).toContain(':disabled="creating || !hasCreateForm"')
    expect(source).toContain('@click="resetCreateForm"')
  })

  test('new user dialog clears stale form values when closed', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function handleCreateOpenChange(nextOpen: boolean)')
    expect(source).toContain('createOpen.value = nextOpen')
    expect(source).toContain('resetCreateForm()')
    expect(source).toContain('<Dialog :open="createOpen" @update:open="handleCreateOpenChange">')
  })

  test('new user dialog cannot be closed while creating', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain(`function handleCreateOpenChange(nextOpen: boolean) {
  if (creating.value && !nextOpen) {
    return
  }
  createOpen.value = nextOpen`)
  })

  test('usage ranking has an independent search query', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const usageQ = ref('')")
    expect(source).toContain("const usageUserId = ref('')")
    expect(source).toContain("const usageSort = ref('-actual_total_bytes')")
    expect(source).toContain('const usageQuery = computed(() => ({')
    expect(source).toContain('q: usageQ.value.trim()')
    expect(source).toContain('user_id: usageUserId.value.trim()')
    expect(source).toContain('sort: usageSort.value')
    expect(source).toContain('controlApi.userUsage(usageQuery.value)')
    expect(source).toContain('watch([usageQ, usageUserId, usageSort], () => usage.refresh())')
    expect(source).toContain('<SearchToolbar v-model="usageQ"')
    expect(source).toContain('<Input id="usage-user-id" v-model="usageUserId" :placeholder="t(\'common.exactUserId\')" :aria-label="t(\'common.exactUserId\')" />')
    expect(source).toContain('<Select v-model="usageSort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'usage.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem value="-session_count">{{ t(\'usage.sessions\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="-relay_quota_granted_bytes">{{ t(\'usage.grantedQuota\') }}</SelectItem>')
    expect(source).toContain('usage.data.value?.total')
  })

  test('user row role selector has a descriptive accessible label', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain(':model-value="user.role"')
    expect(source).toContain(':disabled="hasBusyAction"')
    expect(source).toContain('runBusyAction(`role:${user.user_id}`')
    expect(source).toContain(":aria-label=\"t('adminUser.roleSelectAria', { email: user.email })\"")
    expect(source).toContain('@update:model-value="setRole(user, String($event))"')
  })

  test('usage ranking filters can be reset to defaults', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasUsageFilters = computed(() =>')
    expect(source).toContain("usageQ.value.trim() !== ''")
    expect(source).toContain("usageUserId.value.trim() !== ''")
    expect(source).toContain("usageSort.value !== '-actual_total_bytes'")
    expect(source).toContain('function resetUsageFilters()')
    expect(source).toContain("usageQ.value = ''")
    expect(source).toContain("usageUserId.value = ''")
    expect(source).toContain("usageSort.value = '-actual_total_bytes'")
    expect(source).toContain(':disabled="!hasUsageFilters"')
    expect(source).toContain('@click="resetUsageFilters"')
  })

  test('user detail supports resource search and plan overrides', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const detailQ = ref('')")
    expect(source).toContain('const detailControllers = computed(() =>')
    expect(source).toContain('const detailDevices = computed(() =>')
    expect(source).toContain('detailQ.value.trim().toLowerCase()')
    expect(source).toContain('<SearchToolbar v-model="detailQ"')
    expect(source).toContain('v-for="controller in detailControllers"')
    expect(source).toContain('v-for="device in detailDevices"')
    expect(source).toContain('detailControllers.length === 0')
    expect(source).toContain('detailDevices.length === 0')
    expect(source).toContain('const planOverrideForm = reactive({')
    expect(source).toContain('createUserPlanUpdateRequest(planOverrideForm)')
    expect(source).toContain('controlApi.updateUserPlan(')
    expect(source).toContain('async function savePlanOverride()')
    expect(source).toContain('@submit.prevent="savePlanOverride"')
  })

  test('user detail resource search can be reset independently', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasDetailFilters = computed(() =>')
    expect(source).toContain("detailQ.value.trim() !== ''")
    expect(source).toContain('function resetDetailFilters()')
    expect(source).toContain("detailQ.value = ''")
    expect(source).toContain(':disabled="!hasDetailFilters"')
    expect(source).toContain('@click="resetDetailFilters"')
  })

  test('user detail plan controls are labelled and overrides can be restored', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import ByteUnitInput from '@/components/control/ByteUnitInput.vue'")
    expect(source).toContain("import DurationUnitInput from '@/components/control/DurationUnitInput.vue'")
    expect(source).toContain('for="detail-plan-id"')
    expect(source).toContain('id="detail-plan-id"')
    expect(source).toContain('<ByteUnitInput')
    expect(source).toContain('id="override-max-bps"')
    expect(source).toContain('v-model="planOverrideForm.relay_limits.max_bps"')
    expect(source).toContain(':label="t(\'plan.bandwidthLimit\')"')
    expect(source).toContain('rate')
    expect(source).toContain('<DurationUnitInput')
    expect(source).toContain('id="override-duration"')
    expect(source).toContain('v-model="planOverrideForm.relay_limits.max_duration_sec"')
    expect(source).toContain(':label="t(\'plan.sessionDuration\')"')
    expect(source).toContain('id="override-traffic-quota"')
    expect(source).toContain('v-model="planOverrideForm.relay_limits.traffic_quota_bytes"')
    expect(source).toContain(':label="t(\'plan.trafficQuota\')"')
    expect(source).not.toContain('流量配额 bytes')
    expect(source).not.toContain('会话时长秒')
    expect(source).toContain('type="button"')
    expect(source).toContain(':disabled="savingPlanOverride"')
    expect(source).toContain('@click="syncPlanOverrideForm"')
    expect(source).toContain("{{ t('adminUser.restoreCurrentPlan') }}")
  })

  test('user detail dialog clears selected user and detail filters when closed', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function handleDetailOpenChange(nextOpen: boolean)')
    expect(source).toContain('detailOpen.value = nextOpen')
    expect(source).toContain('selectedUser.value = null')
    expect(source).toContain('resetDetailFilters()')
    expect(source).toContain('<Dialog :open="detailOpen" @update:open="handleDetailOpenChange">')
  })

  test('user detail dialog cannot be closed while plan changes are saving', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain(`function handleDetailOpenChange(nextOpen: boolean) {
  if ((assigningPlan.value || savingPlanOverride.value) && !nextOpen) {
    return
  }
  detailOpen.value = nextOpen`)
  })

  test('user detail plan summary shows the same limit labels as plan management', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain(":label=\"t('plan.bandwidthLimit')\"")
    expect(source).toContain(":label=\"t('adminUser.maxStreams')\"")
    expect(source).toContain(":label=\"t('plan.sessionDuration')\"")
    expect(source).toContain('formatDuration(selectedUser.plan.relay_limits.max_duration_sec)')
  })

  test('users view visible copy is localized', () => {
    const source = readFileSync(new URL('../admin/AdminUsersView.vue', import.meta.url), 'utf8')

    expect(source).toContain("success: t('adminUser.toast.created')")
    expect(source).toContain("error: t('adminUser.toast.createFailed')")
    expect(source).toContain("success: user.enabled ? t('adminUser.toast.disabled') : t('adminUser.toast.enabled')")
    expect(source).toContain("error: t('adminUser.toast.statusFailed')")
    expect(source).toContain("success: t('adminUser.toast.roleUpdated')")
    expect(source).toContain("error: t('adminUser.toast.roleFailed')")
    expect(source).toContain("success: t('usage.toast.reset')")
    expect(source).toContain("error: t('usage.toast.resetFailed')")
    expect(source).toContain("success: t('adminUser.toast.planAssigned')")
    expect(source).toContain("success: t('adminUser.toast.planOverrideSaved')")
    expect(source).toContain("{{ t('adminUser.create') }}")
    expect(source).toContain('<DialogTitle>{{ t(\'adminUser.create\') }}</DialogTitle>')
    expect(source).toContain('<DialogDescription>{{ t(\'adminUser.createDescription\') }}</DialogDescription>')
    expect(source).toContain('<Label for="new-email">{{ t(\'common.email\') }}</Label>')
    expect(source).toContain('<Label for="new-name">{{ t(\'auth.displayName\') }}</Label>')
    expect(source).toContain('<Label for="new-password">{{ t(\'auth.password\') }}</Label>')
    expect(source).toContain('<Label for="new-role">{{ t(\'common.role\') }}</Label>')
    expect(source).toContain('<Label for="new-enabled">{{ t(\'adminUser.enabledAccount\') }}</Label>')
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'adminUser.searchPlaceholder\')"')
    expect(source).toContain("{{ t('adminUser.total', { total: users.data.value?.total ?? 0 }) }}")
    expect(source).toContain(':empty-title="t(\'adminUser.empty\')"')
    expect(source).toContain('<TableHead>{{ t(\'common.user\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'common.role\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'plan.table.plan\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'adminUser.resources\') }}</TableHead>')
    expect(source).toContain("{{ t('adminUser.detail') }}")
    expect(source).toContain("{{ user.enabled ? t('common.disable') : t('common.enable') }}")
    expect(source).toContain("t('adminUser.resourceSummary', { controllers: user.controller_count, devices: user.device_count })")
    expect(source).toContain('<DialogTitle>{{ t(\'adminUser.detailTitle\') }}</DialogTitle>')
    expect(source).toContain('<DialogDescription>{{ t(\'adminUser.detailDescription\') }}</DialogDescription>')
    expect(source).toContain("{{ t('adminUser.loadingDetail') }}")
    expect(source).toContain('<Label for="detail-plan-id">{{ t(\'adminUser.currentPlan\') }}</Label>')
    expect(source).toContain("{{ t('adminUser.assignPlan') }}")
    expect(source).toContain("{{ t('adminUser.planOverride') }}")
    expect(source).toContain("{{ t('adminUser.saveOverride') }}")
    expect(source).toContain('<SearchToolbar v-model="detailQ" :placeholder="t(\'adminUser.detailSearchPlaceholder\')" :refresh-label="t(\'adminUser.reloadDetail\')"')
    expect(source).toContain(':title="t(\'controller.empty\')"')
    expect(source).toContain(':title="t(\'device.empty\')"')
    expect(source).toContain('<SearchToolbar v-model="usageQ" :placeholder="t(\'usage.searchPlaceholder\')"')
    expect(source).toContain("{{ t('usage.total', { total: usage.data.value?.total ?? 0 }) }}")
    expect(source).toContain(':empty-title="t(\'usage.empty\')"')
    expect(source).toContain(':title="t(\'usage.resetTitle\')"')
    expect(source).toContain(":description=\"t('usage.resetDescription', { email: row.email })\"")
    expect(source).not.toContain('用户已创建')
    expect(source).not.toContain('新建用户')
    expect(source).not.toContain('暂无用户')
    expect(source).not.toContain('重置用量周期')
  })
})
