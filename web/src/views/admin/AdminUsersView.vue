<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Eye, HardDrive, Loader2, Plus, RotateCcw, ShieldCheck, UserCog } from 'lucide-vue-next'

import ByteUnitInput from '@/components/control/ByteUnitInput.vue'
import DurationUnitInput from '@/components/control/DurationUnitInput.vue'
import ConfirmAction from '@/components/layout/ConfirmAction.vue'
import EmptyState from '@/components/layout/EmptyState.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Badge } from '@/components/ui/badge'
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
import { Switch } from '@/components/ui/switch'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { formatBytes, formatDuration, formatEpoch } from '@/lib/control/format'
import { createUserPlanAssignmentRequest, createUserPlanUpdateRequest } from '@/lib/control/forms'
import { formatDeviceStatus, formatEnabledLabel, formatRoleLabel } from '@/lib/control/labels'
import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '@/lib/control/select-filter'
import type { ControlRole, UserDetail, UserSummary } from '@/lib/control/types'

const q = ref('')
const role = ref('')
const enabled = ref('')
const sort = ref('email')
const usageQ = ref('')
const usageUserId = ref('')
const usageSort = ref('-actual_total_bytes')
const detailQ = ref('')
const createOpen = ref(false)
const detailOpen = ref(false)
const creating = ref(false)
const detailLoading = ref(false)
const assigningPlan = ref(false)
const savingPlanOverride = ref(false)
const selectedUser = ref<UserDetail | null>(null)
const { t } = useI18n()
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
let detailRequestId = 0
const createForm = reactive({
  email: '',
  password: '',
  display_name: '',
  role: 'user' as ControlRole,
  enabled: true,
})
const planForm = reactive({ plan_id: '' })
const planOverrideForm = reactive({
  plan_id: '',
  name: '',
  max_controller_devices: 0,
  relay_limits: {
    max_bps: 0,
    max_streams: 0,
    max_duration_sec: 0,
    traffic_quota_bytes: 0,
  },
})

const usersQuery = computed(() => ({
  q: q.value.trim(),
  role: role.value,
  enabled: enabled.value === '' ? undefined : enabled.value === 'true',
  limit: 100,
  sort: sort.value,
}))
const usageQuery = computed(() => ({
  q: usageQ.value.trim(),
  user_id: usageUserId.value.trim(),
  limit: 100,
  sort: usageSort.value,
}))
const hasUserFilters = computed(() =>
  q.value.trim() !== '' ||
  role.value !== '' ||
  enabled.value !== '' ||
  sort.value !== 'email',
)
const hasUsageFilters = computed(() =>
  usageQ.value.trim() !== '' ||
  usageUserId.value.trim() !== '' ||
  usageSort.value !== '-actual_total_bytes',
)
const hasDetailFilters = computed(() =>
  detailQ.value.trim() !== '',
)
const hasCreateForm = computed(() =>
  createForm.email.trim() !== '' &&
  createForm.password.trim() !== '' &&
  createForm.display_name.trim() !== '',
)
const users = useAsyncData(() => controlApi.users(usersQuery.value))
const usage = useAsyncData(() => controlApi.userUsage(usageQuery.value))
const plans = useAsyncData(() => controlApi.planCatalog({ limit: 100, sort: 'plan_id' }))
const detailControllers = computed(() => {
  const keyword = detailQ.value.trim().toLowerCase()
  const controllers = selectedUser.value?.controllers ?? []
  if (!keyword) {
    return controllers
  }
  return controllers.filter((controller) =>
    [controller.name, controller.client_id, controller.user_id].some((value) =>
      value.toLowerCase().includes(keyword),
    ),
  )
})
const detailDevices = computed(() => {
  const keyword = detailQ.value.trim().toLowerCase()
  const devices = selectedUser.value?.devices ?? []
  if (!keyword) {
    return devices
  }
  return devices.filter((device) =>
    [
      device.name,
      device.device_id,
      device.user_id,
      device.status,
      device.agent_version,
    ].some((value) => value.toLowerCase().includes(keyword)),
  )
})

watch([q, role, enabled, sort], () => users.refresh())
watch([usageQ, usageUserId, usageSort], () => usage.refresh())

function resetUserFilters() {
  q.value = ''
  role.value = ''
  enabled.value = ''
  sort.value = 'email'
}

function resetUsageFilters() {
  usageQ.value = ''
  usageUserId.value = ''
  usageSort.value = '-actual_total_bytes'
}

function resetDetailFilters() {
  detailQ.value = ''
}

function resetCreateForm() {
  createForm.email = ''
  createForm.password = ''
  createForm.display_name = ''
  createForm.role = 'user'
  createForm.enabled = true
}

function handleCreateOpenChange(nextOpen: boolean) {
  if (creating.value && !nextOpen) {
    return
  }
  createOpen.value = nextOpen
  if (!nextOpen) {
    resetCreateForm()
  }
}

function syncPlanOverrideForm() {
  if (!selectedUser.value) {
    return
  }
  const plan = selectedUser.value.plan
  planOverrideForm.plan_id = plan.plan_id
  planOverrideForm.name = plan.name
  planOverrideForm.max_controller_devices = plan.max_controller_devices
  planOverrideForm.relay_limits = { ...plan.relay_limits }
}

async function createUser() {
  if (creating.value || !hasCreateForm.value) {
    return
  }
  creating.value = true
  try {
    await runWithToast(
      async () => {
        await controlApi.createUser(createForm)
        createOpen.value = false
        resetCreateForm()
        await users.refresh()
      },
      {
        success: t('adminUser.toast.created'),
        error: t('adminUser.toast.createFailed'),
      },
    )
  } finally {
    creating.value = false
  }
}

async function toggleUser(user: UserSummary) {
  await runBusyAction(`toggle:${user.user_id}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.updateUserStatus(user.user_id, { enabled: !user.enabled })
        await users.refresh()
      },
      {
        success: user.enabled ? t('adminUser.toast.disabled') : t('adminUser.toast.enabled'),
        error: t('adminUser.toast.statusFailed'),
      },
    )
  })
}

async function setRole(user: UserSummary, role: string) {
  await runBusyAction(`role:${user.user_id}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.updateUserRole(user.user_id, { role: role as ControlRole })
        await users.refresh()
        if (selectedUser.value?.user.user_id === user.user_id) {
          selectedUser.value.user.role = role as ControlRole
        }
      },
      {
        success: t('adminUser.toast.roleUpdated'),
        error: t('adminUser.toast.roleFailed'),
      },
    )
  })
}

async function resetUsage(userId: string) {
  await runBusyAction(`reset:${userId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.resetUserUsage(userId)
        await usage.refresh()
      },
      {
        success: t('usage.toast.reset'),
        error: t('usage.toast.resetFailed'),
      },
    )
  })
}

async function openUserDetail(user: UserSummary) {
  const requestId = ++detailRequestId
  detailOpen.value = true
  detailLoading.value = true
  selectedUser.value = null
  detailQ.value = ''
  try {
    const detail = await controlApi.user(user.user_id)
    if (requestId !== detailRequestId || !detailOpen.value) {
      return
    }
    selectedUser.value = detail
    planForm.plan_id = detail.plan.plan_id
    syncPlanOverrideForm()
  } finally {
    if (requestId === detailRequestId) {
      detailLoading.value = false
    }
  }
}

function handleDetailOpenChange(nextOpen: boolean) {
  if ((assigningPlan.value || savingPlanOverride.value) && !nextOpen) {
    return
  }
  detailOpen.value = nextOpen
  if (!nextOpen) {
    detailRequestId += 1
    detailLoading.value = false
    selectedUser.value = null
    planForm.plan_id = ''
    resetDetailFilters()
  }
}

async function assignPlan() {
  if (!selectedUser.value || assigningPlan.value || !planForm.plan_id) {
    return
  }
  assigningPlan.value = true
  try {
    await runWithToast(
      async () => {
        const plan = await controlApi.assignUserPlan(
          selectedUser.value!.user.user_id,
          createUserPlanAssignmentRequest(planForm),
        )
        selectedUser.value!.plan = plan
        selectedUser.value!.user.plan_id = plan.plan_id
        syncPlanOverrideForm()
        await users.refresh()
        await usage.refresh()
      },
      {
        success: t('adminUser.toast.planAssigned'),
        error: t('adminUser.toast.planAssignFailed'),
      },
    )
  } finally {
    assigningPlan.value = false
  }
}

async function savePlanOverride() {
  if (!selectedUser.value || savingPlanOverride.value || !planOverrideForm.plan_id || !planOverrideForm.name) {
    return
  }
  savingPlanOverride.value = true
  try {
    await runWithToast(
      async () => {
        const plan = await controlApi.updateUserPlan(
          selectedUser.value!.user.user_id,
          createUserPlanUpdateRequest(planOverrideForm),
        )
        selectedUser.value!.plan = plan
        selectedUser.value!.user.plan_id = plan.plan_id
        planForm.plan_id = plan.plan_id
        syncPlanOverrideForm()
        await users.refresh()
        await usage.refresh()
      },
      {
        success: t('adminUser.toast.planOverrideSaved'),
        error: t('adminUser.toast.planOverrideFailed'),
      },
    )
  } finally {
    savingPlanOverride.value = false
  }
}

</script>

<template>
  <main class="page-container">
    <PageSection :title="t('route.admin.users.title')" :description="t('route.admin.users.description')">
      <template #actions>
        <Dialog :open="createOpen" @update:open="handleCreateOpenChange">
          <DialogTrigger as-child>
            <Button @click="resetCreateForm">
              <Plus class="size-4" />
              {{ t('adminUser.create') }}
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{{ t('adminUser.create') }}</DialogTitle>
              <DialogDescription>{{ t('adminUser.createDescription') }}</DialogDescription>
            </DialogHeader>
            <form class="grid gap-4" @submit.prevent="createUser">
              <div class="grid gap-2">
                <Label for="new-email">{{ t('common.email') }}</Label>
                <Input id="new-email" v-model="createForm.email" required type="email" autocomplete="email" />
              </div>
              <div class="grid gap-2">
                <Label for="new-name">{{ t('auth.displayName') }}</Label>
                <Input id="new-name" v-model="createForm.display_name" required autocomplete="name" />
              </div>
              <div class="grid gap-2">
                <Label for="new-password">{{ t('auth.password') }}</Label>
                <Input id="new-password" v-model="createForm.password" required type="password" minlength="8" autocomplete="new-password" />
              </div>
              <div class="grid gap-2">
                <Label for="new-role">{{ t('common.role') }}</Label>
                <Select v-model="createForm.role">
                  <SelectTrigger id="new-role"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="user">{{ formatRoleLabel('user', t) }}</SelectItem>
                    <SelectItem value="admin">{{ formatRoleLabel('admin', t) }}</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div class="flex items-center justify-between rounded-md border p-3">
                <Label for="new-enabled">{{ t('adminUser.enabledAccount') }}</Label>
                <Switch id="new-enabled" v-model:checked="createForm.enabled" />
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" :disabled="creating" @click="resetCreateForm">
                  {{ t('common.reset') }}
                </Button>
                <Button type="submit" :disabled="creating || !hasCreateForm">
                  <Loader2 v-if="creating" class="animate-spin" />
                  {{ t('common.create') }}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </template>

      <div class="grid gap-4">
        <SearchToolbar v-model="q" :placeholder="t('adminUser.searchPlaceholder')" :loading="users.loading.value" @refresh="users.refresh" />
        <div class="grid gap-3 lg:grid-cols-[1fr_1fr_1fr_auto_auto] lg:items-center">
          <Select :model-value="selectFilterValue(role)" @update:model-value="role = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('adminUser.roleFilter')">
              <SelectValue :placeholder="t('adminUser.allRoles')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('adminUser.allRoles') }}</SelectItem>
              <SelectItem value="user">{{ formatRoleLabel('user', t) }}</SelectItem>
              <SelectItem value="admin">{{ formatRoleLabel('admin', t) }}</SelectItem>
            </SelectContent>
          </Select>
          <Select :model-value="selectFilterValue(enabled)" @update:model-value="enabled = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('adminUser.accountStatusFilter')">
              <SelectValue :placeholder="t('common.allStatus')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('common.allStatus') }}</SelectItem>
              <SelectItem value="true">{{ formatEnabledLabel(true, t) }}</SelectItem>
              <SelectItem value="false">{{ formatEnabledLabel(false, t) }}</SelectItem>
            </SelectContent>
          </Select>
          <Select v-model="sort">
            <SelectTrigger :aria-label="t('adminUser.sortLabel')">
              <SelectValue :placeholder="t('common.sort')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="email">{{ t('common.email') }}</SelectItem>
              <SelectItem value="-email">{{ t('adminUser.sortEmailDesc') }}</SelectItem>
              <SelectItem value="role">{{ t('common.role') }}</SelectItem>
              <SelectItem value="enabled">{{ t('adminUser.sortEnabled') }}</SelectItem>
              <SelectItem value="controller_count">{{ t('adminUser.sortControllers') }}</SelectItem>
              <SelectItem value="device_count">{{ t('adminUser.sortDevices') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasUserFilters" @click="resetUserFilters">
            {{ t('common.reset') }}
          </Button>
          <p class="text-sm text-muted-foreground lg:text-right">
            {{ t('adminUser.total', { total: users.data.value?.total ?? 0 }) }}
          </p>
        </div>
        <ResponsiveTable
          :items="users.data.value?.items ?? []"
          :loading="users.loading.value"
          :error="users.error.value"
          :empty-title="t('adminUser.empty')"
          @retry="users.refresh"
        >
          <template #head>
            <TableRow>
              <TableHead>{{ t('common.user') }}</TableHead>
              <TableHead>{{ t('common.role') }}</TableHead>
              <TableHead>{{ t('plan.table.plan') }}</TableHead>
              <TableHead>{{ t('adminUser.resources') }}</TableHead>
              <TableHead>{{ t('common.status') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="user in users.data.value?.items ?? []" :key="user.user_id">
              <TableCell>
                <div class="font-medium">{{ user.display_name }}</div>
                <div class="text-xs text-muted-foreground">{{ user.email }}</div>
              </TableCell>
              <TableCell>
                <Select
                  :model-value="user.role"
                  :disabled="hasBusyAction"
                  @update:model-value="setRole(user, String($event))"
                >
                  <SelectTrigger class="w-28" :aria-label="t('adminUser.roleSelectAria', { email: user.email })"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="user">{{ formatRoleLabel('user', t) }}</SelectItem>
                    <SelectItem value="admin">{{ formatRoleLabel('admin', t) }}</SelectItem>
                  </SelectContent>
                </Select>
              </TableCell>
              <TableCell><Badge variant="outline">{{ user.plan_id }}</Badge></TableCell>
              <TableCell>{{ t('adminUser.resourceSummary', { controllers: user.controller_count, devices: user.device_count }) }}</TableCell>
              <TableCell>
                <Badge :variant="user.enabled ? 'success' : 'secondary'">
                  {{ formatEnabledLabel(user.enabled, t) }}
                </Badge>
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button variant="outline" size="sm" :disabled="hasBusyAction" @click="openUserDetail(user)">
                    <Eye class="size-4" />
                    {{ t('adminUser.detail') }}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="hasBusyAction"
                    @click="toggleUser(user)"
                  >
                    <Loader2 v-if="isBusy('toggle', user.user_id)" class="animate-spin" />
                    {{ user.enabled ? t('common.disable') : t('common.enable') }}
                  </Button>
                </div>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="user in users.data.value?.items ?? []" :key="user.user_id" class="rounded-md border p-4">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <p class="truncate font-medium">{{ user.display_name }}</p>
                  <p class="truncate text-sm text-muted-foreground">{{ user.email }}</p>
                </div>
                <Badge :variant="user.enabled ? 'success' : 'secondary'">{{ formatEnabledLabel(user.enabled, t) }}</Badge>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('common.userId')" :value="user.user_id" />
                <InfoRow :label="t('common.role')" :value="formatRoleLabel(user.role, t)" />
                <InfoRow :label="t('plan.table.plan')" :value="user.plan_id" />
                <InfoRow :label="t('adminUser.resources')" :value="t('adminUser.resourceSummary', { controllers: user.controller_count, devices: user.device_count })" />
              </div>
              <div class="mt-3 grid grid-cols-2 gap-2">
                <Button variant="outline" size="sm" :disabled="hasBusyAction" @click="openUserDetail(user)">
                  <Eye class="size-4" />
                  {{ t('adminUser.detail') }}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="hasBusyAction"
                  @click="toggleUser(user)"
                >
                  <Loader2 v-if="isBusy('toggle', user.user_id)" class="animate-spin" />
                  {{ user.enabled ? t('common.disable') : t('common.enable') }}
                </Button>
              </div>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>

    <Dialog :open="detailOpen" @update:open="handleDetailOpenChange">
      <DialogContent class="max-h-[90vh] overflow-y-auto sm:max-w-4xl">
        <DialogHeader>
          <DialogTitle>{{ t('adminUser.detailTitle') }}</DialogTitle>
          <DialogDescription>{{ t('adminUser.detailDescription') }}</DialogDescription>
        </DialogHeader>

        <div v-if="detailLoading" class="flex min-h-48 items-center justify-center text-sm text-muted-foreground">
          <Loader2 class="mr-2 size-4 animate-spin" />
          {{ t('adminUser.loadingDetail') }}
        </div>

        <div v-else-if="selectedUser" class="grid gap-5">
          <div class="grid gap-3 sm:grid-cols-2">
            <InfoRow :label="t('common.email')" :value="selectedUser.user.email" />
            <InfoRow :label="t('common.userId')" :value="selectedUser.user.user_id" />
            <InfoRow :label="t('auth.displayName')" :value="selectedUser.user.display_name" />
            <InfoRow :label="t('common.status')" :value="formatEnabledLabel(selectedUser.user.enabled, t)" />
          </div>

          <div class="grid gap-3 rounded-md border p-4">
            <div class="flex flex-col gap-3 sm:flex-row sm:items-end">
              <div class="grid min-w-0 flex-1 gap-2">
                <Label for="detail-plan-id">{{ t('adminUser.currentPlan') }}</Label>
                <Select v-model="planForm.plan_id">
                  <SelectTrigger id="detail-plan-id"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="plan in plans.data.value?.items ?? []"
                      :key="plan.plan_id"
                      :value="plan.plan_id"
                    >
                      {{ plan.name }} / {{ plan.plan_id }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <Button :disabled="assigningPlan || !planForm.plan_id" @click="assignPlan">
                <Loader2 v-if="assigningPlan" class="animate-spin" />
                {{ t('adminUser.assignPlan') }}
              </Button>
            </div>
            <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
              <InfoRow :label="t('common.planId')" :value="selectedUser.plan.plan_id" />
              <InfoRow :label="t('adminUser.maxControllers')" :value="selectedUser.plan.max_controller_devices" />
              <InfoRow :label="t('plan.bandwidthLimit')" :value="`${formatBytes(selectedUser.plan.relay_limits.max_bps)}/s`" />
              <InfoRow :label="t('adminUser.maxStreams')" :value="selectedUser.plan.relay_limits.max_streams" />
              <InfoRow :label="t('plan.sessionDuration')" :value="formatDuration(selectedUser.plan.relay_limits.max_duration_sec)" />
              <InfoRow :label="t('plan.trafficQuota')" :value="formatBytes(selectedUser.plan.relay_limits.traffic_quota_bytes)" />
            </div>
            <form class="grid gap-4 border-t pt-4" @submit.prevent="savePlanOverride">
              <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <p class="font-medium">{{ t('adminUser.planOverride') }}</p>
                <div class="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    :disabled="savingPlanOverride"
                    @click="syncPlanOverrideForm"
                  >
                    {{ t('adminUser.restoreCurrentPlan') }}
                  </Button>
                  <Button
                    type="submit"
                    class="sm:w-auto"
                    :disabled="savingPlanOverride || !planOverrideForm.plan_id || !planOverrideForm.name"
                  >
                    <Loader2 v-if="savingPlanOverride" class="animate-spin" />
                    {{ t('adminUser.saveOverride') }}
                  </Button>
                </div>
              </div>
              <div class="grid gap-3 sm:grid-cols-2">
                <div class="grid gap-2">
                  <Label for="override-plan-id">{{ t('common.planId') }}</Label>
                  <Input id="override-plan-id" v-model="planOverrideForm.plan_id" required />
                </div>
                <div class="grid gap-2">
                  <Label for="override-plan-name">{{ t('common.name') }}</Label>
                  <Input id="override-plan-name" v-model="planOverrideForm.name" required />
                </div>
                <div class="grid gap-2">
                  <Label for="override-max-controllers">{{ t('plan.maxControllers') }}</Label>
                  <Input
                    id="override-max-controllers"
                    v-model.number="planOverrideForm.max_controller_devices"
                    type="number"
                    min="0"
                  />
                </div>
                <div class="grid gap-2">
                  <Label for="override-max-streams">{{ t('plan.maxStreams') }}</Label>
                  <Input
                    id="override-max-streams"
                    v-model.number="planOverrideForm.relay_limits.max_streams"
                    type="number"
                    min="0"
                  />
                </div>
                <ByteUnitInput
                  id="override-max-bps"
                  v-model="planOverrideForm.relay_limits.max_bps"
                  :label="t('plan.bandwidthLimit')"
                  rate
                />
                <DurationUnitInput
                  id="override-duration"
                  v-model="planOverrideForm.relay_limits.max_duration_sec"
                  :label="t('plan.sessionDuration')"
                />
                <ByteUnitInput
                  id="override-traffic-quota"
                  v-model="planOverrideForm.relay_limits.traffic_quota_bytes"
                  class="sm:col-span-2"
                  :label="t('plan.trafficQuota')"
                />
              </div>
            </form>
          </div>

          <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
            <SearchToolbar v-model="detailQ" :placeholder="t('adminUser.detailSearchPlaceholder')" :refresh-label="t('adminUser.reloadDetail')" @refresh="openUserDetail(selectedUser.user)" />
            <Button variant="outline" :disabled="!hasDetailFilters" @click="resetDetailFilters">
              {{ t('common.reset') }}
            </Button>
          </div>

          <div class="grid gap-4 lg:grid-cols-2">
            <div class="rounded-md border p-4">
              <div class="mb-3 flex items-center gap-2">
                <ShieldCheck class="size-4 text-muted-foreground" />
                <p class="font-medium">{{ t('nav.center.controllers') }}</p>
                <Badge variant="outline">{{ detailControllers.length }} / {{ selectedUser.controllers.length }}</Badge>
              </div>
              <div class="grid gap-2">
                <div
                  v-for="controller in detailControllers"
                  :key="controller.client_id"
                  class="rounded-md bg-muted p-3 text-sm"
                >
                  <p class="font-medium">{{ controller.name }}</p>
                  <p class="break-all text-muted-foreground">{{ controller.client_id }}</p>
                </div>
                <EmptyState v-if="detailControllers.length === 0" :title="t('controller.empty')" />
              </div>
            </div>

            <div class="rounded-md border p-4">
              <div class="mb-3 flex items-center gap-2">
                <HardDrive class="size-4 text-muted-foreground" />
                <p class="font-medium">{{ t('common.device') }}</p>
                <Badge variant="outline">{{ detailDevices.length }} / {{ selectedUser.devices.length }}</Badge>
              </div>
              <div class="grid gap-2">
                <div
                  v-for="device in detailDevices"
                  :key="device.device_id"
                  class="rounded-md bg-muted p-3 text-sm"
                >
                  <div class="flex items-center justify-between gap-2">
                    <p class="font-medium">{{ device.name }}</p>
                    <Badge :variant="device.status === 'online' ? 'success' : 'secondary'">{{ formatDeviceStatus(device.status, t) }}</Badge>
                  </div>
                  <p class="break-all text-muted-foreground">{{ device.device_id }}</p>
                </div>
                <EmptyState v-if="detailDevices.length === 0" :title="t('device.empty')" />
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>

    <PageSection :title="t('section.admin.users.usageTitle')" :description="t('section.admin.users.usageDescription')">
      <div class="grid gap-4">
        <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_220px_220px_auto]">
          <SearchToolbar v-model="usageQ" :placeholder="t('usage.searchPlaceholder')" :loading="usage.loading.value" @refresh="usage.refresh" />
          <Input id="usage-user-id" v-model="usageUserId" :placeholder="t('common.exactUserId')" :aria-label="t('common.exactUserId')" />
          <Select v-model="usageSort">
            <SelectTrigger :aria-label="t('usage.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="-actual_total_bytes">{{ t('usage.totalTraffic') }}</SelectItem>
              <SelectItem value="-session_count">{{ t('usage.sessions') }}</SelectItem>
              <SelectItem value="-relay_quota_granted_bytes">{{ t('usage.grantedQuota') }}</SelectItem>
              <SelectItem value="email">{{ t('common.email') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasUsageFilters" @click="resetUsageFilters">
            {{ t('common.reset') }}
          </Button>
        </div>
        <p class="text-sm text-muted-foreground sm:text-right">{{ t('usage.total', { total: usage.data.value?.total ?? 0 }) }}</p>
        <ResponsiveTable
          :items="usage.data.value?.items ?? []"
          :loading="usage.loading.value"
          :error="usage.error.value"
          :empty-title="t('usage.empty')"
          @retry="usage.refresh"
        >
          <template #head>
            <TableRow>
              <TableHead>{{ t('common.user') }}</TableHead>
              <TableHead>{{ t('plan.table.plan') }}</TableHead>
              <TableHead>{{ t('usage.sessions') }}</TableHead>
              <TableHead>{{ t('usage.traffic') }}</TableHead>
              <TableHead>{{ t('usage.period') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="row in usage.data.value?.items ?? []" :key="row.user_id">
              <TableCell>{{ row.email }}</TableCell>
              <TableCell>{{ row.plan_id }}</TableCell>
              <TableCell>{{ row.session_count }}</TableCell>
              <TableCell>{{ formatBytes(row.actual_total_bytes) }}</TableCell>
              <TableCell>{{ formatEpoch(row.current_period_started_epoch_sec) }}</TableCell>
              <TableCell class="text-right">
                <ConfirmAction
                  :title="t('usage.resetTitle')"
                  :description="t('usage.resetDescription', { email: row.email })"
                  :confirm-text="t('common.reset')"
                  variant="outline"
                  :icon="RotateCcw"
                  :disabled="hasBusyAction"
                  :loading="isBusy('reset', row.user_id)"
                  @confirm="resetUsage(row.user_id)"
                >
                  {{ t('common.reset') }}
                </ConfirmAction>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="row in usage.data.value?.items ?? []" :key="row.user_id" class="rounded-md border p-4">
              <div class="flex items-center gap-2">
                <UserCog class="size-4 text-muted-foreground" />
                <p class="min-w-0 truncate font-medium">{{ row.email }}</p>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('plan.table.plan')" :value="row.plan_id" />
                <InfoRow :label="t('usage.sessions')" :value="row.session_count" />
                <InfoRow :label="t('usage.actualTraffic')" :value="formatBytes(row.actual_total_bytes)" />
                <InfoRow :label="t('usage.period')" :value="formatEpoch(row.current_period_started_epoch_sec)" />
              </div>
              <ConfirmAction
                class="mt-3 w-full"
                :title="t('usage.resetTitle')"
                :description="t('usage.resetDescription', { email: row.email })"
                :confirm-text="t('common.reset')"
                variant="outline"
                :icon="RotateCcw"
                :disabled="hasBusyAction"
                :loading="isBusy('reset', row.user_id)"
                @confirm="resetUsage(row.user_id)"
              >
                {{ t('usage.resetPeriod') }}
              </ConfirmAction>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>
  </main>
</template>
