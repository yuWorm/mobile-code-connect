import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('admin plan filters', () => {
  test('plan catalog searches by query and shows totals', () => {
    const source = readFileSync(new URL('../admin/AdminPlansView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const q = ref('')")
    expect(source).toContain("const sort = ref('plan_id')")
    expect(source).toContain('const plansQuery = computed(() => ({')
    expect(source).toContain('q: q.value.trim()')
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('watch([q, sort], () => plans.refresh())')
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'plan.searchPlaceholder\')"')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'plan.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem value="name">{{ t(\'plan.sortName\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="-max_controller_devices">{{ t(\'plan.sortControllerLimit\') }}</SelectItem>')
    expect(source).toContain("t('plan.total'")
  })

  test('plan catalog filters can be reset to defaults', () => {
    const source = readFileSync(new URL('../admin/AdminPlansView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasPlanFilters = computed(() =>')
    expect(source).toContain("q.value.trim() !== ''")
    expect(source).toContain("sort.value !== 'plan_id'")
    expect(source).toContain('function resetPlanFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("sort.value = 'plan_id'")
    expect(source).toContain(':disabled="!hasPlanFilters"')
    expect(source).toContain('@click="resetPlanFilters"')
  })

  test('plan dialog form exposes labelled fields and an inline reset action', () => {
    const source = readFileSync(new URL('../admin/AdminPlansView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasPlanForm = computed(() =>')
    expect(source).toContain("form.plan_id.trim() !== ''")
    expect(source).toContain("form.name.trim() !== ''")
    expect(source).toContain('for="plan-id"')
    expect(source).toContain('id="plan-id"')
    expect(source).toContain('for="plan-name"')
    expect(source).toContain('id="plan-name"')
    expect(source).toContain('for="plan-max-controller-devices"')
    expect(source).toContain('id="plan-max-controller-devices"')
    expect(source).toContain('for="plan-max-streams"')
    expect(source).toContain('id="plan-max-streams"')
    expect(source).toContain("import ByteUnitInput from '@/components/control/ByteUnitInput.vue'")
    expect(source).toContain("import DurationUnitInput from '@/components/control/DurationUnitInput.vue'")
    expect(source).toContain('<ByteUnitInput')
    expect(source).toContain('id="plan-max-bps"')
    expect(source).toContain('v-model="form.relay_limits.max_bps"')
    expect(source).toContain(":label=\"t('plan.bandwidthLimit')\"")
    expect(source).toContain('rate')
    expect(source).toContain('<DurationUnitInput')
    expect(source).toContain('id="plan-max-duration"')
    expect(source).toContain('v-model="form.relay_limits.max_duration_sec"')
    expect(source).toContain(":label=\"t('plan.sessionDuration')\"")
    expect(source).toContain('id="plan-traffic-quota"')
    expect(source).toContain('v-model="form.relay_limits.traffic_quota_bytes"')
    expect(source).toContain(":label=\"t('plan.trafficQuota')\"")
    expect(source).not.toContain('流量配额 bytes')
    expect(source).not.toContain('id="plan-traffic-quota-bytes"')
    expect(source).not.toContain('会话时长秒')
    expect(source).toContain(`async function savePlan() {
  if (saving.value || !hasPlanForm.value) {
    return
  }`)
    expect(source).toContain('type="button"')
    expect(source).toContain(':disabled="saving"')
    expect(source).toContain(':disabled="saving || !hasPlanForm"')
    expect(source).toContain('@click="resetForm"')
  })

  test('plan dialog clears stale form state when closed', () => {
    const source = readFileSync(new URL('../admin/AdminPlansView.vue', import.meta.url), 'utf8')

    expect(source).toContain('function handlePlanOpenChange(nextOpen: boolean)')
    expect(source).toContain('open.value = nextOpen')
    expect(source).toContain('resetForm()')
    expect(source).toContain('<Dialog :open="open" @update:open="handlePlanOpenChange">')
  })

  test('plan dialog cannot be closed while saving', () => {
    const source = readFileSync(new URL('../admin/AdminPlansView.vue', import.meta.url), 'utf8')

    expect(source).toContain(`function handlePlanOpenChange(nextOpen: boolean) {
  if (saving.value && !nextOpen) {
    return
  }
  open.value = nextOpen`)
  })

  test('plan visible copy and limit labels are localized', () => {
    const source = readFileSync(new URL('../admin/AdminPlansView.vue', import.meta.url), 'utf8')

    expect(source).toContain("success: (plan) => t('plan.toast.saved', { name: plan.name || plan.plan_id })")
    expect(source).toContain("error: t('plan.toast.saveFailed')")
    expect(source).toContain("{{ t('plan.create') }}")
    expect(source).toContain("<DialogTitle>{{ t('plan.dialogTitle') }}</DialogTitle>")
    expect(source).toContain("<DialogDescription>{{ t('plan.dialogDescription') }}</DialogDescription>")
    expect(source).toContain("{{ t('common.reset') }}")
    expect(source).toContain("{{ t('common.save') }}")
    expect(source).toContain(":empty-title=\"t('plan.empty')\"")
    expect(source).toContain("<TableHead>{{ t('plan.table.plan') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('plan.controllers') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('plan.relayLimits') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('plan.trafficQuota') }}</TableHead>")
    expect(source).toContain("{{ t('plan.relayLimitSummary'")
    expect(source).toContain(":label=\"t('plan.bandwidthLimit')\"")
    expect(source).toContain(":label=\"t('plan.maxStreams')\"")
    expect(source).toContain("{{ t('common.edit') }}")
    expect(source).not.toContain('限速')
    expect(source).not.toContain('新建套餐')
    expect(source).not.toContain('暂无套餐模板')
  })
})
