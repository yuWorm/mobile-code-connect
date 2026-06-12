import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('admin session filters', () => {
  test('sessions view supports exact user and device filters', () => {
    const source = readFileSync(new URL('../admin/AdminSessionsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const userId = ref('')")
    expect(source).toContain("const deviceId = ref('')")
    expect(source).toContain("const sort = ref('-expire_at')")
    expect(source).toContain('user_id: userId.value.trim()')
    expect(source).toContain('device_id: deviceId.value.trim()')
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('watch([q, status, userId, deviceId, sort], () => sessions.refresh())')
    expect(source).toContain('<Input v-model="userId"')
    expect(source).toContain('<Input v-model="deviceId"')
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'session.searchPlaceholder\')"')
    expect(source).toContain('<Input v-model="userId" :placeholder="t(\'common.exactUserId\')" :aria-label="t(\'common.exactUserId\')" />')
    expect(source).toContain('<Input v-model="deviceId" :placeholder="t(\'common.exactDeviceId\')" :aria-label="t(\'common.exactDeviceId\')" />')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'session.filterStatus\')"><SelectValue :placeholder="t(\'common.allStatus\')" /></SelectTrigger>')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'session.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem value="status">{{ t(\'common.status\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="user_email">{{ t(\'session.sortUserEmail\') }}</SelectItem>')
    expect(source).toContain("t('session.total'")
  })

  test('sessions view shows localized status labels', () => {
    const source = readFileSync(new URL('../admin/AdminSessionsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { formatSessionStatus } from '@/lib/control/labels'")
    expect(source).toContain('<SelectItem value="pending">{{ formatSessionStatus(\'pending\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="claimed">{{ formatSessionStatus(\'claimed\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="bound">{{ formatSessionStatus(\'bound\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="closed">{{ formatSessionStatus(\'closed\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="expired">{{ formatSessionStatus(\'expired\', t) }}</SelectItem>')
    expect(source).toContain('formatSessionStatus(session.status, t)')
    expect(source).toContain('formatSessionStatus(selectedSession.status, t)')
    expect(source).not.toContain('{{ session.status }}')
    expect(source).not.toContain('{{ selectedSession.status }}')
  })

  test('sessions view can reset all filters back to defaults', () => {
    const source = readFileSync(new URL('../admin/AdminSessionsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasSessionFilters = computed(() =>')
    expect(source).toContain("sort.value !== '-expire_at'")
    expect(source).toContain('function resetSessionFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("status.value = ''")
    expect(source).toContain("userId.value = ''")
    expect(source).toContain("deviceId.value = ''")
    expect(source).toContain("sort.value = '-expire_at'")
    expect(source).toContain(':disabled="!hasSessionFilters"')
    expect(source).toContain('@click="resetSessionFilters"')
  })

  test('session detail dialog clears selected session when closed', () => {
    const source = readFileSync(new URL('../admin/AdminSessionsView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function handleDetailOpenChange(nextOpen: boolean)')
    expect(source).toContain('detailOpen.value = nextOpen')
    expect(source).toContain('selectedSession.value = null')
    expect(source).toContain('<Dialog :open="detailOpen" @update:open="handleDetailOpenChange">')
  })

  test('sessions view visible copy is localized', () => {
    const source = readFileSync(new URL('../admin/AdminSessionsView.vue', import.meta.url), 'utf8')

    expect(source).toContain("success: t('session.toast.closed')")
    expect(source).toContain("error: t('session.toast.closeFailed')")
    expect(source).toContain("{{ t('common.reset') }}")
    expect(source).toContain(":empty-title=\"t('session.empty')\"")
    expect(source).toContain("<TableHead>{{ t('target.session') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.user') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('session.targetService') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.status') }}</TableHead>")
    expect(source).toContain("<TableHead class=\"text-right\">{{ t('common.actions') }}</TableHead>")
    expect(source).toContain("{{ t('common.details') }}")
    expect(source).toContain(":title=\"t('session.closeTitle')\"")
    expect(source).toContain(":description=\"t('session.closeDescription', { id: session.session_id })\"")
    expect(source).toContain(":confirm-text=\"t('common.close')\"")
    expect(source).toContain("<DialogTitle>{{ t('session.detailTitle') }}</DialogTitle>")
    expect(source).toContain("{{ t('session.userPanel') }}")
    expect(source).toContain("{{ t('session.targetPanel') }}")
    expect(source).toContain("{{ t('session.connectionInfo') }}")
    expect(source).not.toContain('会话已关闭')
    expect(source).not.toContain('关闭会话')
    expect(source).not.toContain('暂无会话')
  })
})
