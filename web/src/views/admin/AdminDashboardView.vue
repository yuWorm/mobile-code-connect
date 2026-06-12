<script setup lang="ts">
import { Activity, HardDrive, Loader2, Network, RefreshCw, Users } from 'lucide-vue-next'

import EmptyState from '@/components/layout/EmptyState.vue'
import ErrorState from '@/components/layout/ErrorState.vue'
import LoadingState from '@/components/layout/LoadingState.vue'
import PageSection from '@/components/layout/PageSection.vue'
import StatCard from '@/components/layout/StatCard.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { useI18n } from '@/composables/useI18n'
import { useAsyncData } from '@/composables/useAsyncData'
import { controlApi } from '@/lib/control/client'
import { formatBytes, formatEpoch } from '@/lib/control/format'
import { formatAuditTargetType } from '@/lib/control/labels'

const { data, loading, error, refresh } = useAsyncData(() => controlApi.dashboard())
const { t } = useI18n()
</script>

<template>
  <main class="page-container">
    <section class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div class="min-w-0">
        <h1 class="text-xl font-semibold">{{ t('dashboard.admin.title') }}</h1>
        <p class="mt-1 text-sm text-muted-foreground">{{ t('dashboard.admin.description') }}</p>
      </div>
      <Button variant="outline" :disabled="loading" @click="refresh">
        <Loader2 v-if="loading" class="animate-spin" />
        <RefreshCw v-else class="size-4" />
        {{ t('dashboard.admin.refresh') }}
      </Button>
    </section>

    <LoadingState v-if="loading && !data" :label="t('dashboard.admin.loading')" />
    <ErrorState v-else-if="error && !data" :message="error" @retry="refresh" />
    <template v-else-if="data">
      <section class="responsive-grid">
        <StatCard
          :label="t('dashboard.admin.users')"
          :value="data.users.total"
          :description="t('dashboard.admin.usersDescription', { enabled: data.users.enabled, admins: data.users.admins })"
          :icon="Users"
        />
        <StatCard
          :label="t('dashboard.admin.devices')"
          :value="data.devices.total"
          :description="t('dashboard.admin.devicesDescription', { online: data.devices.online })"
          :icon="HardDrive"
        />
        <StatCard
          :label="t('dashboard.admin.sessions')"
          :value="data.sessions.total"
          :description="t('dashboard.admin.sessionsDescription', { pending: data.sessions.pending, bound: data.sessions.bound })"
          :icon="Activity"
        />
        <StatCard
          :label="t('dashboard.admin.relays')"
          :value="data.relays.total"
          :description="t('dashboard.admin.relaysDescription', { healthy: data.relays.healthy, unhealthy: data.relays.unhealthy })"
          :icon="Network"
        />
      </section>

      <div class="grid gap-6 xl:grid-cols-[1fr_420px]">
        <PageSection :title="t('dashboard.admin.usageTitle')" :description="t('dashboard.admin.usageDescription')">
          <div class="grid gap-4 sm:grid-cols-3">
            <div class="rounded-md border p-4">
              <p class="text-sm text-muted-foreground">{{ t('dashboard.admin.totalTraffic') }}</p>
              <p class="mt-2 text-2xl font-semibold">{{ formatBytes(data.usage.actual_total_bytes) }}</p>
            </div>
            <div class="rounded-md border p-4">
              <p class="text-sm text-muted-foreground">{{ t('dashboard.admin.uplink') }}</p>
              <p class="mt-2 text-2xl font-semibold">{{ formatBytes(data.usage.actual_uplink_bytes) }}</p>
            </div>
            <div class="rounded-md border p-4">
              <p class="text-sm text-muted-foreground">{{ t('dashboard.admin.downlink') }}</p>
              <p class="mt-2 text-2xl font-semibold">{{ formatBytes(data.usage.actual_downlink_bytes) }}</p>
            </div>
          </div>
        </PageSection>

        <PageSection :title="t('dashboard.admin.recentAudit')" :description="t('dashboard.admin.recentAuditDescription')">
          <EmptyState
            v-if="data.recent_audit_logs.length === 0"
            :title="t('dashboard.admin.emptyAudit')"
            :description="t('dashboard.admin.emptyAuditDescription')"
          />
          <Table v-else>
            <TableHeader>
              <TableRow>
                <TableHead>{{ t('dashboard.admin.action') }}</TableHead>
                <TableHead>{{ t('dashboard.admin.target') }}</TableHead>
                <TableHead>{{ t('dashboard.admin.time') }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="log in data.recent_audit_logs" :key="log.audit_id">
                <TableCell>
                  <Badge variant="outline">{{ log.action }}</Badge>
                </TableCell>
                <TableCell class="max-w-36 truncate">{{ formatAuditTargetType(log.target_type, t) }} / {{ log.target_id }}</TableCell>
                <TableCell class="whitespace-nowrap">{{ formatEpoch(log.created_epoch_sec) }}</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </PageSection>
      </div>
    </template>
  </main>
</template>
