import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('UserDevicesView', () => {
  test('constrains service action labels in responsive layouts', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain('class="max-w-full min-w-0"')
    expect(source).toContain('class="truncate"')
  })

  test('filters loaded device services locally', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const serviceQ = ref('')")
    expect(source).toContain('function filteredServices(deviceId: string)')
    expect(source).toContain('serviceQ.value.trim().toLowerCase()')
    expect(source).toContain('service.service_id')
    expect(source).toContain('String(service.target_port)')
    expect(source).toContain('const serviceRefreshing = computed(() =>')
    expect(source).toContain('async function refreshLoadedServices()')
    expect(source).toContain('Object.keys(services.value)')
    expect(source).toContain('controlApi.deviceServices(deviceId)')
    expect(source).toContain('<SearchToolbar v-model="serviceQ"')
    expect(source).toContain('@refresh="refreshLoadedServices"')
    expect(source).toContain('v-for="service in filteredServices(device.device_id)"')
    expect(source).toContain('filteredServices(device.device_id).length === 0')
  })

  test('device and service filter reset actions are available', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function resetDeviceFilters()')
    expect(source).toContain('function resetServiceFilters()')
    expect(source).toContain(':disabled="!hasDeviceFilters"')
    expect(source).toContain(':disabled="!hasServiceFilters"')
  })

  test('created session result can be dismissed to clear sensitive tokens', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { copyToClipboard } from '@/lib/control/clipboard'")
    expect(source).toContain('await copyToClipboard(value)')
    expect(source).not.toContain('navigator.clipboard.writeText')
    expect(source).toContain('function dismissCreatedSession()')
    expect(source).toContain('createdSession.value = null')
    expect(source).toContain('copiedSessionField.value = null')
    expect(source).toContain(':aria-label="t(\'device.closeSessionResult\')"')
    expect(source).toContain('@click="dismissCreatedSession"')
    expect(source).toContain('<X class="size-4" />')
    expect(source).toContain('<span class="sr-only">{{ t(\'device.closeSessionResult\') }}</span>')
  })

  test('creating a session clears stale session results first', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain(`async function createSession() {
  if (creatingSession.value || !sessionForm.client_id) {
    return
  }
  creatingSession.value = true
  createdSession.value = null
  copiedSessionField.value = null`)
  })

  test('loading services is guarded per device to avoid duplicate requests', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain('if (serviceLoading.value[deviceId]) {')
    expect(source).toContain('return')
    expect(source).toContain('serviceLoading.value[deviceId] = true')
    expect(source).toContain('serviceLoading.value[deviceId] = false')
  })

  test('session dialog controller selection is labelled and can be reset', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function resetSessionForm()')
    expect(source).toContain("sessionForm.client_id = availableControllers.value[0]?.client_id ?? ''")
    expect(source).toContain('createdSession.value = null')
    expect(source).toContain('copiedSessionField.value = null')
    expect(source).toContain('for="session-controller-id"')
    expect(source).toContain('id="session-controller-id"')
    expect(source).toContain('type="button"')
    expect(source).toContain(':disabled="creatingSession"')
    expect(source).toContain('@click="resetSessionForm"')
  })

  test('session dialog clears selected service and sensitive tokens when closed', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function resetSessionDialogState()')
    expect(source).toContain('selectedDevice.value = null')
    expect(source).toContain('selectedService.value = null')
    expect(source).toContain("sessionForm.device_id = ''")
    expect(source).toContain("sessionForm.service_id = ''")
    expect(source).toContain('createdSession.value = null')
    expect(source).toContain('copiedSessionField.value = null')
    expect(source).toContain('function handleSessionOpenChange(nextOpen: boolean)')
    expect(source).toContain('<Dialog :open="sessionOpen" @update:open="handleSessionOpenChange">')
  })

  test('session dialog cannot be closed while creating a session', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain(`function handleSessionOpenChange(nextOpen: boolean) {
  if (creatingSession.value && !nextOpen) {
    return
  }
  sessionOpen.value = nextOpen`)
  })

  test('session dialog and result copy is localized', () => {
    const source = readFileSync(new URL('../UserDevicesView.vue', import.meta.url), 'utf8')

    expect(source).toContain('<DialogTitle>{{ t(\'device.createSessionTitle\') }}</DialogTitle>')
    expect(source).toContain('<Label for="session-controller-id">{{ t(\'device.controller\') }}</Label>')
    expect(source).toContain('<SelectTrigger id="session-controller-id"><SelectValue :placeholder="t(\'device.selectController\')" /></SelectTrigger>')
    expect(source).toContain("{{ t('device.noControllers') }}")
    expect(source).toContain("{{ t('common.reset') }}")
    expect(source).toContain("{{ t('device.createSession') }}")
    expect(source).toContain("{{ t('device.sessionResult') }}")
    expect(source).toContain("{{ copiedSessionField === 'bundle' ? t('common.copied') : t('device.copyBundle') }}")
    expect(source).toContain(':label="t(\'common.expiresAt\')"')
    expect(source).toContain(':label="t(\'device.p2pCert\')"')
    expect(source).toContain("createdSession.agent_p2p_cert_der?.length ? t('device.included') : '-'")
    expect(source).toContain("{{ copiedSessionField === 'access' ? t('common.copied') : t('common.copy') }}")
    expect(source).toContain("{{ copiedSessionField === 'relay' ? t('common.copied') : t('common.copy') }}")
    expect(source).not.toContain('创建访问会话')
    expect(source).not.toContain('还没有控制器')
    expect(source).not.toContain('会话结果')
    expect(source).not.toContain('复制凭据包')
  })
})
