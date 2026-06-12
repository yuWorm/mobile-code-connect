<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ScrollText } from 'lucide-vue-next'

import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useI18n } from '@/composables/useI18n'
import { controlApi } from '@/lib/control/client'
import { formatEpoch } from '@/lib/control/format'
import { formatAuditTargetType, formatRoleLabel } from '@/lib/control/labels'
import { ALL_SELECT_VALUE, normalizeSelectFilterValue, selectFilterValue } from '@/lib/control/select-filter'

const q = ref('')
const action = ref('')
const targetType = ref('')
const sort = ref('-created_epoch_sec')
const { t } = useI18n()
const query = computed(() => ({
  q: q.value.trim(),
  action: action.value,
  target_type: targetType.value,
  limit: 100,
  sort: sort.value,
}))
const hasAuditFilters = computed(() =>
  q.value.trim() !== '' ||
  action.value !== '' ||
  targetType.value !== '' ||
  sort.value !== '-created_epoch_sec',
)
const logs = useAsyncData(() => controlApi.auditLogs(query.value))
watch([q, action, targetType, sort], () => logs.refresh())

function resetAuditFilters() {
  q.value = ''
  action.value = ''
  targetType.value = ''
  sort.value = '-created_epoch_sec'
}
</script>

<template>
  <main class="page-container">
    <PageSection :title="t('route.admin.audit.title')" :description="t('route.admin.audit.description')">
      <div class="grid gap-4">
        <SearchToolbar v-model="q" :placeholder="t('audit.searchPlaceholder')" :loading="logs.loading.value" @refresh="logs.refresh" />
        <div class="grid gap-3 md:grid-cols-[1fr_1fr_1fr_auto_auto] md:items-center">
          <Select :model-value="selectFilterValue(action)" @update:model-value="action = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('audit.filterAction')">
              <SelectValue :placeholder="t('audit.allActions')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('audit.allActions') }}</SelectItem>
              <SelectItem value="user.create">user.create</SelectItem>
              <SelectItem value="user.status.update">user.status.update</SelectItem>
              <SelectItem value="user.role.update">user.role.update</SelectItem>
              <SelectItem value="plan.assign">plan.assign</SelectItem>
              <SelectItem value="relay.register">relay.register</SelectItem>
              <SelectItem value="relay.delete">relay.delete</SelectItem>
              <SelectItem value="relay_credential.rotate">relay_credential.rotate</SelectItem>
              <SelectItem value="server_credential.issue">server_credential.issue</SelectItem>
              <SelectItem value="oauth_identity.unlink">oauth_identity.unlink</SelectItem>
              <SelectItem value="auth.password.change">auth.password.change</SelectItem>
            </SelectContent>
          </Select>
          <Select :model-value="selectFilterValue(targetType)" @update:model-value="targetType = normalizeSelectFilterValue($event)">
            <SelectTrigger :aria-label="t('audit.filterTarget')">
              <SelectValue :placeholder="t('audit.allTargets')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem :value="ALL_SELECT_VALUE">{{ t('audit.allTargets') }}</SelectItem>
              <SelectItem value="user">{{ formatAuditTargetType('user', t) }}</SelectItem>
              <SelectItem value="plan">{{ formatAuditTargetType('plan', t) }}</SelectItem>
              <SelectItem value="relay">{{ formatAuditTargetType('relay', t) }}</SelectItem>
              <SelectItem value="relay_credential">{{ formatAuditTargetType('relay_credential', t) }}</SelectItem>
              <SelectItem value="server_credential">{{ formatAuditTargetType('server_credential', t) }}</SelectItem>
              <SelectItem value="oauth_identity">{{ formatAuditTargetType('oauth_identity', t) }}</SelectItem>
              <SelectItem value="session">{{ formatAuditTargetType('session', t) }}</SelectItem>
            </SelectContent>
          </Select>
          <Select v-model="sort">
            <SelectTrigger :aria-label="t('audit.sortLabel')">
              <SelectValue :placeholder="t('common.sort')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="-created_epoch_sec">{{ t('audit.sortCreatedDesc') }}</SelectItem>
              <SelectItem value="created_epoch_sec_asc">{{ t('audit.sortCreatedAsc') }}</SelectItem>
              <SelectItem value="action">{{ t('common.action') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasAuditFilters" @click="resetAuditFilters">
            {{ t('common.reset') }}
          </Button>
          <p class="text-sm text-muted-foreground md:text-right">
            {{ t('audit.total', { total: logs.data.value?.total ?? 0 }) }}
          </p>
        </div>
        <ResponsiveTable
          :items="logs.data.value?.items ?? []"
          :loading="logs.loading.value"
          :error="logs.error.value"
          :empty-title="t('audit.empty')"
          @retry="logs.refresh"
        >
          <template #head>
            <TableRow>
              <TableHead>{{ t('common.action') }}</TableHead>
              <TableHead>{{ t('audit.actor') }}</TableHead>
              <TableHead>{{ t('common.target') }}</TableHead>
              <TableHead>{{ t('common.message') }}</TableHead>
              <TableHead>{{ t('common.time') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="log in logs.data.value?.items ?? []" :key="log.audit_id">
              <TableCell><Badge variant="outline">{{ log.action }}</Badge></TableCell>
              <TableCell>{{ log.actor_subject }}<div class="text-xs text-muted-foreground">{{ formatRoleLabel(log.actor_role, t) }}</div></TableCell>
              <TableCell>{{ formatAuditTargetType(log.target_type, t) }} / {{ log.target_id }}</TableCell>
              <TableCell class="max-w-md">{{ log.message }}</TableCell>
              <TableCell class="whitespace-nowrap">{{ formatEpoch(log.created_epoch_sec) }}</TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="log in logs.data.value?.items ?? []" :key="log.audit_id" class="rounded-md border p-4">
              <div class="flex items-start gap-2">
                <ScrollText class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                <div class="min-w-0">
                  <Badge variant="outline">{{ log.action }}</Badge>
                  <p class="mt-2 text-sm">{{ log.message }}</p>
                </div>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('audit.actor')" :value="log.actor_subject" />
                <InfoRow :label="t('common.target')" :value="`${formatAuditTargetType(log.target_type, t)} / ${log.target_id}`" />
                <InfoRow :label="t('common.time')" :value="formatEpoch(log.created_epoch_sec)" />
              </div>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>
  </main>
</template>
