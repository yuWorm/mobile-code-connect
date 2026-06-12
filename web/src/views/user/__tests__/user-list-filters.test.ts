import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('user list filters', () => {
  test('devices view filters accessible devices locally by search and status', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const q = ref('')")
    expect(source).toContain("const status = ref('')")
    expect(source).toContain("const sort = ref('name')")
    expect(source).toContain('const filteredDevices = computed(() =>')
    expect(source).toContain('device.status === status.value')
    expect(source).toContain('const sorted = [...items].sort((left: Device, right: Device) =>')
    expect(source).toContain("if (sort.value === 'status')")
    expect(source).toContain("if (sort.value === 'agent_version')")
    expect(source).toContain('<SearchToolbar v-model="q"')
    expect(source).toContain('<Select :model-value="selectFilterValue(status)" @update:model-value="status = normalizeSelectFilterValue($event)">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'device.filterStatus\')"><SelectValue :placeholder="t(\'common.allStatus\')" /></SelectTrigger>')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'device.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem :value="ALL_SELECT_VALUE">{{ t(\'common.allStatus\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="online">{{ formatDeviceStatus(\'online\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="offline">{{ formatDeviceStatus(\'offline\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="agent_version">{{ t(\'device.agentVersion\') }}</SelectItem>')
    expect(source).toContain('filteredDevices.length')
  })

  test('devices view shows localized device and session result labels', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { formatDeviceStatus } from '@/lib/control/labels'")
    expect(source).toContain('formatDeviceStatus(device.status, t)')
    expect(source).toContain("createdSession.agent_p2p_cert_der?.length ? t('device.included') : '-'")
    expect(source).not.toContain('{{ device.status }}')
    expect(source).not.toContain("'included'")
  })

  test('devices view filter controls can be reset to defaults', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasDeviceFilters = computed(() =>')
    expect(source).toContain("q.value.trim() !== ''")
    expect(source).toContain("status.value !== ''")
    expect(source).toContain("sort.value !== 'name'")
    expect(source).toContain('function resetDeviceFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("status.value = ''")
    expect(source).toContain("sort.value = 'name'")
    expect(source).toContain(':disabled="!hasDeviceFilters"')
    expect(source).toContain('@click="resetDeviceFilters"')
  })

  test('loaded service filter can be reset independently', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasServiceFilters = computed(() =>')
    expect(source).toContain("serviceQ.value.trim() !== ''")
    expect(source).toContain('function resetServiceFilters()')
    expect(source).toContain("serviceQ.value = ''")
    expect(source).toContain(':disabled="!hasServiceFilters"')
    expect(source).toContain('@click="resetServiceFilters"')
  })

  test('controllers view searches through backend query and shows totals', () => {
    const source = readFileSync(new URL('../UserControllersView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const q = ref('')")
    expect(source).toContain("const sort = ref('client_id')")
    expect(source).toContain('const controllersQuery = computed(() => ({')
    expect(source).toContain('q: q.value.trim()')
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('watch([q, sort], () => controllers.refresh())')
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'controller.searchPlaceholder\')"')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'controller.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem value="name">{{ t(\'controller.sortName\') }}</SelectItem>')
    expect(source).toContain("t('controller.total'")
  })

  test('controllers view filters can be reset to defaults', () => {
    const source = readFileSync(new URL('../UserControllersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasControllerFilters = computed(() =>')
    expect(source).toContain("q.value.trim() !== ''")
    expect(source).toContain("sort.value !== 'client_id'")
    expect(source).toContain('function resetControllerFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("sort.value = 'client_id'")
    expect(source).toContain(':disabled="!hasControllerFilters"')
    expect(source).toContain('@click="resetControllerFilters"')
  })

  test('controllers create dialog resets stale form values before opening', () => {
    const source = readFileSync(new URL('../UserControllersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function resetControllerForm()')
    expect(source).toContain("form.client_id = ''")
    expect(source).toContain("form.name = ''")
    expect(source).toContain('@click="resetControllerForm"')
    expect(source).toContain('resetControllerForm()')
  })

  test('controllers create dialog exposes labelled fields and inline reset action', () => {
    const source = readFileSync(new URL('../UserControllersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasControllerForm = computed(() =>')
    expect(source).toContain("form.client_id.trim() !== ''")
    expect(source).toContain("form.name.trim() !== ''")
    expect(source).toContain('for="controller-client-id"')
    expect(source).toContain('id="controller-client-id"')
    expect(source).toContain('for="controller-name"')
    expect(source).toContain('id="controller-name"')
    expect(source).toContain('type="button"')
    expect(source).toContain(':disabled="saving"')
    expect(source).toContain(':disabled="saving || !hasControllerForm"')
    expect(source).toContain('@click="resetControllerForm"')
  })

  test('controllers create dialog clears stale form values when closed', () => {
    const source = readFileSync(new URL('../UserControllersView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function handleControllerOpenChange(nextOpen: boolean)')
    expect(source).toContain('open.value = nextOpen')
    expect(source).toContain('resetControllerForm()')
    expect(source).toContain('<Dialog :open="open" @update:open="handleControllerOpenChange">')
  })

  test('controllers create dialog cannot be closed while saving', () => {
    const source = readFileSync(new URL('../UserControllersView.vue', import.meta.url), 'utf8')

    expect(source).toContain(`function handleControllerOpenChange(nextOpen: boolean) {
  if (saving.value && !nextOpen) {
    return
  }
  open.value = nextOpen`)
  })

  test('controllers view visible copy is localized', () => {
    const source = readFileSync(new URL('../UserControllersView.vue', import.meta.url), 'utf8')

    expect(source).toContain("success: t('controller.toast.created')")
    expect(source).toContain("error: t('controller.toast.createFailed')")
    expect(source).toContain("success: t('controller.toast.removed')")
    expect(source).toContain("error: t('controller.toast.removeFailed')")
    expect(source).toContain("{{ t('controller.register') }}")
    expect(source).toContain("<DialogTitle>{{ t('controller.register') }}</DialogTitle>")
    expect(source).toContain("<DialogDescription>{{ t('controller.registerDescription') }}</DialogDescription>")
    expect(source).toContain("{{ t('common.reset') }}")
    expect(source).toContain("{{ t('common.register') }}")
    expect(source).toContain(":empty-title=\"t('controller.empty')\"")
    expect(source).toContain("<TableHead>{{ t('controller.table.controller') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.user') }}</TableHead>")
    expect(source).toContain("<TableHead class=\"text-right\">{{ t('common.actions') }}</TableHead>")
    expect(source).toContain(":title=\"t('controller.removeTitle')\"")
    expect(source).toContain(":description=\"t('controller.removeDescription', { name: controller.name })\"")
    expect(source).toContain(":confirm-text=\"t('common.remove')\"")
    expect(source).toContain("{{ t('common.remove') }}")
    expect(source).toContain(":label=\"t('common.clientId')\"")
    expect(source).toContain(":label=\"t('common.user')\"")
    expect(source).not.toContain('控制器已注册')
    expect(source).not.toContain('注册控制器')
    expect(source).not.toContain('移除控制器')
    expect(source).not.toContain('暂无控制器')
  })

  test('devices view visible copy is localized', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain("success: (items) => (items.length > 0 ? t('device.toast.servicesLoaded', { total: items.length }) : t('device.toast.noServices'))")
    expect(source).toContain("error: t('device.toast.loadServicesFailed')")
    expect(source).toContain("success: t('device.toast.sessionCreated')")
    expect(source).toContain("error: t('device.toast.sessionFailed')")
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'device.searchPlaceholder\')"')
    expect(source).toContain("{{ t('device.accessibleTotal', { shown: filteredDevices.length, total: devices.data.value?.length ?? 0 }) }}")
    expect(source).toContain('<SearchToolbar v-model="serviceQ"')
    expect(source).toContain(':placeholder="t(\'device.serviceSearchPlaceholder\')"')
    expect(source).toContain(':empty-title="t(\'device.emptyAccessible\')"')
    expect(source).toContain('<TableHead>{{ t(\'common.device\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'device.agentVersion\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'common.status\') }}</TableHead>')
    expect(source).toContain('<TableHead>{{ t(\'common.service\') }}</TableHead>')
    expect(source).toContain('<TableHead class="text-right">{{ t(\'common.actions\') }}</TableHead>')
    expect(source).toContain("{{ t('device.noServices') }}")
    expect(source).toContain("{{ t('device.notLoaded') }}")
    expect(source).toContain("{{ t('device.service') }}")
    expect(source).toContain("{{ t('device.loadServices') }}")
    expect(source).toContain(":label=\"t('common.deviceId')\"")
    expect(source).toContain("title=\"t('device.noServices')\"")
    expect(source).not.toContain('搜索设备 ID、名称、用户或版本')
    expect(source).not.toContain('暂无可访问设备')
    expect(source).not.toContain('无服务')
    expect(source).not.toContain('未加载')
  })
})
