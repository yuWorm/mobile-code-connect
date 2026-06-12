import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('admin audit filters', () => {
  test('audit view filters by action and target type', () => {
    const source = readFileSync(new URL('../admin/AdminAuditView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const action = ref')
    expect(source).toContain('const targetType = ref')
    expect(source).toContain("const sort = ref('-created_epoch_sec')")
    expect(source).toContain('action: action.value')
    expect(source).toContain('target_type: targetType.value')
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('watch([q, action, targetType, sort], () => logs.refresh())')
    expect(source).toContain('<Select :model-value="selectFilterValue(action)" @update:model-value="action = normalizeSelectFilterValue($event)">')
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'audit.searchPlaceholder\')"')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'audit.filterAction\')">')
    expect(source).toContain('<Select :model-value="selectFilterValue(targetType)" @update:model-value="targetType = normalizeSelectFilterValue($event)">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'audit.filterTarget\')">')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'audit.sortLabel\')">')
    expect(source).toContain('<SelectItem value="created_epoch_sec_asc">{{ t(\'audit.sortCreatedAsc\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="action">{{ t(\'common.action\') }}</SelectItem>')
    expect(source).toContain('<SelectItem :value="ALL_SELECT_VALUE">{{ t(\'audit.allActions\') }}</SelectItem>')
    expect(source).toContain('<SelectItem :value="ALL_SELECT_VALUE">{{ t(\'audit.allTargets\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="user">{{ formatAuditTargetType(\'user\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="plan">{{ formatAuditTargetType(\'plan\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="relay">{{ formatAuditTargetType(\'relay\', t) }}</SelectItem>')
    expect(source).toContain('<SelectItem value="session">{{ formatAuditTargetType(\'session\', t) }}</SelectItem>')
    expect(source).toContain("import { formatAuditTargetType, formatRoleLabel } from '@/lib/control/labels'")
    expect(source).toContain('formatRoleLabel(log.actor_role, t)')
    expect(source).toContain('formatAuditTargetType(log.target_type, t)')
    expect(source).toContain('logs.data.value?.total')
  })

  test('audit view visible copy is localized', () => {
    const source = readFileSync(new URL('../admin/AdminAuditView.vue', import.meta.url), 'utf8')

    expect(source).toContain("{{ t('common.reset') }}")
    expect(source).toContain("{{ t('audit.total'")
    expect(source).toContain(":empty-title=\"t('audit.empty')\"")
    expect(source).toContain("<TableHead>{{ t('common.action') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('audit.actor') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.target') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.message') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.time') }}</TableHead>")
    expect(source).toContain(":label=\"t('audit.actor')\"")
    expect(source).toContain(":label=\"t('common.target')\"")
    expect(source).toContain(":label=\"t('common.time')\"")
    expect(source).not.toContain('搜索动作')
    expect(source).not.toContain('暂无审计日志')
    expect(source).not.toContain('操作者')
  })

  test('audit view can reset all filters back to defaults', () => {
    const source = readFileSync(new URL('../admin/AdminAuditView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasAuditFilters = computed(() =>')
    expect(source).toContain("sort.value !== '-created_epoch_sec'")
    expect(source).toContain('function resetAuditFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("action.value = ''")
    expect(source).toContain("targetType.value = ''")
    expect(source).toContain("sort.value = '-created_epoch_sec'")
    expect(source).toContain(':disabled="!hasAuditFilters"')
    expect(source).toContain('@click="resetAuditFilters"')
  })

  test('user dashboard exposes real shortcut links', () => {
    const source = readFileSync(new URL('../user/UserDashboardView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { RouterLink } from 'vue-router'")
    expect(source).toContain("to: '/center/devices'")
    expect(source).toContain("to: '/center/controllers'")
    expect(source).toContain("to: '/center/credentials'")
    expect(source).toContain("to: '/center/account'")
    expect(source).toContain(':to="shortcut.to"')
  })
})
