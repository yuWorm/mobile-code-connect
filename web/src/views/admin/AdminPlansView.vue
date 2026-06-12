<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Loader2, Plus, WalletCards } from 'lucide-vue-next'

import ByteUnitInput from '@/components/control/ByteUnitInput.vue'
import DurationUnitInput from '@/components/control/DurationUnitInput.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { formatBytes, formatDuration } from '@/lib/control/format'
import type { Plan } from '@/lib/control/types'

const q = ref('')
const sort = ref('plan_id')
const open = ref(false)
const saving = ref(false)
const { t } = useI18n()
const form = reactive<Plan>({
  plan_id: '',
  name: '',
  max_controller_devices: 2,
  relay_limits: {
    max_bps: 1_048_576,
    max_streams: 8,
    max_duration_sec: 3600,
    traffic_quota_bytes: 104_857_600,
  },
})

const plansQuery = computed(() => ({
  q: q.value.trim(),
  limit: 100,
  sort: sort.value,
}))
const hasPlanFilters = computed(() =>
  q.value.trim() !== '' ||
  sort.value !== 'plan_id',
)
const hasPlanForm = computed(() =>
  form.plan_id.trim() !== '' &&
  form.name.trim() !== '',
)
const plans = useAsyncData(() => controlApi.planCatalog(plansQuery.value))
watch([q, sort], () => plans.refresh())

function edit(plan: Plan) {
  form.plan_id = plan.plan_id
  form.name = plan.name
  form.max_controller_devices = plan.max_controller_devices
  form.relay_limits = { ...plan.relay_limits }
  open.value = true
}

function resetForm() {
  form.plan_id = ''
  form.name = ''
  form.max_controller_devices = 2
  form.relay_limits = {
    max_bps: 1_048_576,
    max_streams: 8,
    max_duration_sec: 3600,
    traffic_quota_bytes: 104_857_600,
  }
}

function resetPlanFilters() {
  q.value = ''
  sort.value = 'plan_id'
}

function handlePlanOpenChange(nextOpen: boolean) {
  if (saving.value && !nextOpen) {
    return
  }
  open.value = nextOpen
  if (!nextOpen) {
    resetForm()
  }
}

async function savePlan() {
  if (saving.value || !hasPlanForm.value) {
    return
  }
  saving.value = true
  try {
    await runWithToast(
      async () => {
        const plan = await controlApi.updatePlanCatalog({
          plan: {
            plan_id: form.plan_id,
            name: form.name,
            max_controller_devices: Number(form.max_controller_devices),
            relay_limits: {
              max_bps: Number(form.relay_limits.max_bps),
              max_streams: Number(form.relay_limits.max_streams),
              max_duration_sec: Number(form.relay_limits.max_duration_sec),
              traffic_quota_bytes: Number(form.relay_limits.traffic_quota_bytes),
            },
          },
        })
        open.value = false
        resetForm()
        await plans.refresh()
        return plan
      },
      {
        success: (plan) => t('plan.toast.saved', { name: plan.name || plan.plan_id }),
        error: t('plan.toast.saveFailed'),
      },
    )
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <main class="page-container">
    <PageSection :title="t('route.admin.plans.title')" :description="t('route.admin.plans.description')">
      <template #actions>
        <Dialog :open="open" @update:open="handlePlanOpenChange">
          <DialogTrigger as-child>
            <Button @click="resetForm">
              <Plus class="size-4" />
              {{ t('plan.create') }}
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{{ t('plan.dialogTitle') }}</DialogTitle>
              <DialogDescription>{{ t('plan.dialogDescription') }}</DialogDescription>
            </DialogHeader>
            <form class="grid gap-4" @submit.prevent="savePlan">
              <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <div class="grid gap-2">
                  <Label for="plan-id">{{ t('common.planId') }}</Label>
                  <Input id="plan-id" v-model="form.plan_id" required />
                </div>
                <div class="grid gap-2">
                  <Label for="plan-name">{{ t('common.name') }}</Label>
                  <Input id="plan-name" v-model="form.name" required />
                </div>
              </div>
              <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <div class="grid gap-2">
                  <Label for="plan-max-controller-devices">{{ t('plan.maxControllers') }}</Label>
                  <Input id="plan-max-controller-devices" v-model.number="form.max_controller_devices" type="number" min="0" />
                </div>
                <div class="grid gap-2">
                  <Label for="plan-max-streams">{{ t('plan.maxStreams') }}</Label>
                  <Input id="plan-max-streams" v-model.number="form.relay_limits.max_streams" type="number" min="0" />
                </div>
                <ByteUnitInput
                  id="plan-max-bps"
                  v-model="form.relay_limits.max_bps"
                  :label="t('plan.bandwidthLimit')"
                  rate
                />
                <DurationUnitInput
                  id="plan-max-duration"
                  v-model="form.relay_limits.max_duration_sec"
                  :label="t('plan.sessionDuration')"
                />
                <ByteUnitInput
                  id="plan-traffic-quota"
                  v-model="form.relay_limits.traffic_quota_bytes"
                  class="sm:col-span-2"
                  :label="t('plan.trafficQuota')"
                />
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" :disabled="saving" @click="resetForm">
                  {{ t('common.reset') }}
                </Button>
                <Button type="submit" :disabled="saving || !hasPlanForm">
                  <Loader2 v-if="saving" class="animate-spin" />
                  {{ t('common.save') }}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </template>

      <div class="grid gap-4">
        <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_220px_auto]">
          <SearchToolbar v-model="q" :placeholder="t('plan.searchPlaceholder')" :loading="plans.loading.value" @refresh="plans.refresh" />
          <Select v-model="sort">
            <SelectTrigger :aria-label="t('plan.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="plan_id">{{ t('common.planId') }}</SelectItem>
              <SelectItem value="name">{{ t('plan.sortName') }}</SelectItem>
              <SelectItem value="-max_controller_devices">{{ t('plan.sortControllerLimit') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasPlanFilters" @click="resetPlanFilters">
            {{ t('common.reset') }}
          </Button>
        </div>
        <p class="text-sm text-muted-foreground sm:text-right">
          {{ t('plan.total', { total: plans.data.value?.total ?? 0 }) }}
        </p>
        <ResponsiveTable
          :items="plans.data.value?.items ?? []"
          :loading="plans.loading.value"
          :error="plans.error.value"
          :empty-title="t('plan.empty')"
          @retry="plans.refresh"
        >
          <template #head>
            <TableRow>
              <TableHead>{{ t('plan.table.plan') }}</TableHead>
              <TableHead>{{ t('plan.controllers') }}</TableHead>
              <TableHead>{{ t('plan.relayLimits') }}</TableHead>
              <TableHead>{{ t('plan.trafficQuota') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="plan in plans.data.value?.items ?? []" :key="plan.plan_id">
              <TableCell>
                <div class="font-medium">{{ plan.name }}</div>
                <div class="text-xs text-muted-foreground">{{ plan.plan_id }}</div>
              </TableCell>
              <TableCell>{{ plan.max_controller_devices }}</TableCell>
              <TableCell>{{ t('plan.relayLimitSummary', { bandwidth: `${formatBytes(plan.relay_limits.max_bps)}/s`, streams: plan.relay_limits.max_streams, duration: formatDuration(plan.relay_limits.max_duration_sec) }) }}</TableCell>
              <TableCell>{{ formatBytes(plan.relay_limits.traffic_quota_bytes) }}</TableCell>
              <TableCell class="text-right"><Button variant="outline" size="sm" @click="edit(plan)">{{ t('common.edit') }}</Button></TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="plan in plans.data.value?.items ?? []" :key="plan.plan_id" class="rounded-md border p-4">
              <div class="flex items-center gap-2">
                <WalletCards class="size-4 text-muted-foreground" />
                <p class="font-medium">{{ plan.name }}</p>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('common.planId')" :value="plan.plan_id" />
                <InfoRow :label="t('plan.controllers')" :value="plan.max_controller_devices" />
                <InfoRow :label="t('plan.bandwidthLimit')" :value="`${formatBytes(plan.relay_limits.max_bps)}/s`" />
                <InfoRow :label="t('plan.maxStreams')" :value="plan.relay_limits.max_streams" />
                <InfoRow :label="t('plan.sessionDuration')" :value="formatDuration(plan.relay_limits.max_duration_sec)" />
                <InfoRow :label="t('plan.trafficQuota')" :value="formatBytes(plan.relay_limits.traffic_quota_bytes)" />
              </div>
              <Button class="mt-3 w-full" variant="outline" size="sm" @click="edit(plan)">{{ t('common.edit') }}</Button>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>
  </main>
</template>
