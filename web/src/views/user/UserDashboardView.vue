<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import {
  ArrowRight,
  HardDrive,
  KeyRound,
  Loader2,
  RefreshCw,
  Server,
  ShieldCheck,
  UserCircle,
  WalletCards,
} from 'lucide-vue-next'

import ErrorState from '@/components/layout/ErrorState.vue'
import LoadingState from '@/components/layout/LoadingState.vue'
import PageSection from '@/components/layout/PageSection.vue'
import StatCard from '@/components/layout/StatCard.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { useAuth } from '@/composables/useAuth'
import { useAsyncData } from '@/composables/useAsyncData'
import { useI18n } from '@/composables/useI18n'
import { controlApi } from '@/lib/control/client'
import { formatBytes, formatDuration } from '@/lib/control/format'
import { formatRoleLabel } from '@/lib/control/labels'

const { state } = useAuth()
const { t } = useI18n()
const plan = useAsyncData(() => controlApi.currentPlan())
const devices = useAsyncData(() => controlApi.mobileDevices())
const controllers = useAsyncData(() => controlApi.controllers({ limit: 100 }))
const credentials = useAsyncData(() => controlApi.serverCredentials({ limit: 100 }))

const dashboardLoading = computed(() =>
  plan.loading.value ||
  devices.loading.value ||
  controllers.loading.value ||
  credentials.loading.value,
)
const dashboardError = computed(() =>
  plan.error.value ||
  devices.error.value ||
  controllers.error.value ||
  credentials.error.value,
)
const onlineDevices = computed(() => devices.data.value?.filter((device) => device.status === 'online').length ?? 0)
const shortcuts = computed(() => [
  {
    to: '/center/devices',
    title: t('dashboard.user.shortcut.devicesTitle'),
    description: t('dashboard.user.shortcut.devicesDescription'),
    icon: HardDrive,
  },
  {
    to: '/center/controllers',
    title: t('dashboard.user.shortcut.controllersTitle'),
    description: t('dashboard.user.shortcut.controllersDescription'),
    icon: ShieldCheck,
  },
  {
    to: '/center/credentials',
    title: t('dashboard.user.shortcut.credentialsTitle'),
    description: t('dashboard.user.shortcut.credentialsDescription'),
    icon: Server,
  },
  {
    to: '/center/account',
    title: t('dashboard.user.shortcut.accountTitle'),
    description: t('dashboard.user.shortcut.accountDescription'),
    icon: UserCircle,
  },
])

function refreshDashboard() {
  return Promise.all([
    plan.refresh(),
    devices.refresh(),
    controllers.refresh(),
    credentials.refresh(),
  ])
}
</script>

<template>
  <main class="page-container">
    <section class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div class="min-w-0">
        <h1 class="text-xl font-semibold">{{ t('route.center.dashboard.title') }}</h1>
        <p class="mt-1 text-sm text-muted-foreground">{{ t('route.center.dashboard.description') }}</p>
      </div>
      <Button variant="outline" :disabled="dashboardLoading" @click="refreshDashboard">
        <Loader2 v-if="dashboardLoading" class="animate-spin" />
        <RefreshCw v-else class="size-4" />
        {{ t('dashboard.user.refresh') }}
      </Button>
    </section>

    <ErrorState v-if="dashboardError && !dashboardLoading" :message="dashboardError" @retry="refreshDashboard" />

    <section class="responsive-grid">
      <StatCard
        :label="t('dashboard.user.accessibleDevices')"
        :value="devices.data.value?.length ?? 0"
        :description="t('dashboard.user.accessibleDevicesDescription', { online: onlineDevices })"
        :icon="HardDrive"
      />
      <StatCard
        :label="t('dashboard.user.controllers')"
        :value="controllers.data.value?.total ?? 0"
        :description="t('dashboard.user.controllersDescription')"
        :icon="ShieldCheck"
      />
      <StatCard
        :label="t('dashboard.user.credentials')"
        :value="credentials.data.value?.total ?? 0"
        :description="t('dashboard.user.credentialsDescription')"
        :icon="Server"
      />
      <StatCard
        :label="t('dashboard.user.currentRole')"
        :value="formatRoleLabel(state.session?.role, t)"
        :description="state.session?.subject"
        :icon="KeyRound"
      />
    </section>

    <LoadingState v-if="plan.loading.value && !plan.data.value" :label="t('dashboard.user.planLoading')" />
    <PageSection v-else-if="plan.data.value" :title="t('section.user.dashboard.planTitle')" :description="t('section.user.dashboard.planDescription')">
      <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <div class="rounded-md border p-4">
          <div class="flex items-center gap-2">
            <WalletCards class="size-4 text-muted-foreground" />
            <p class="font-medium">{{ plan.data.value.name }}</p>
          </div>
          <Badge class="mt-3" variant="outline">{{ plan.data.value.plan_id }}</Badge>
        </div>
        <div class="rounded-md border p-4">
          <p class="text-sm text-muted-foreground">{{ t('dashboard.user.maxControllers') }}</p>
          <p class="mt-2 text-2xl font-semibold">{{ plan.data.value.max_controller_devices }}</p>
        </div>
        <div class="rounded-md border p-4">
          <p class="text-sm text-muted-foreground">{{ t('dashboard.user.bandwidthLimit') }}</p>
          <p class="mt-2 text-2xl font-semibold">{{ formatBytes(plan.data.value.relay_limits.max_bps) }}/s</p>
          <p class="mt-1 text-xs text-muted-foreground">
            {{ t('dashboard.user.bandwidthMeta', { streams: plan.data.value.relay_limits.max_streams, duration: formatDuration(plan.data.value.relay_limits.max_duration_sec) }) }}
          </p>
        </div>
        <div class="rounded-md border p-4">
          <p class="text-sm text-muted-foreground">{{ t('dashboard.user.trafficQuota') }}</p>
          <p class="mt-2 text-2xl font-semibold">{{ formatBytes(plan.data.value.relay_limits.traffic_quota_bytes) }}</p>
        </div>
      </div>
    </PageSection>

    <PageSection :title="t('section.user.dashboard.shortcutsTitle')" :description="t('section.user.dashboard.shortcutsDescription')">
      <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <RouterLink
          v-for="shortcut in shortcuts"
          :key="shortcut.to"
          :to="shortcut.to"
          class="focus-ring group rounded-md border p-4 transition-colors hover:bg-accent hover:text-accent-foreground"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="flex min-w-0 items-center gap-2">
              <component :is="shortcut.icon" class="size-4 shrink-0 text-muted-foreground group-hover:text-accent-foreground" />
              <p class="truncate font-medium">{{ shortcut.title }}</p>
            </div>
            <ArrowRight class="size-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-accent-foreground" />
          </div>
          <p class="mt-3 text-sm leading-6 text-muted-foreground group-hover:text-accent-foreground">
            {{ shortcut.description }}
          </p>
        </RouterLink>
      </div>
    </PageSection>
  </main>
</template>
