<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Github, KeyRound, Unlink } from 'lucide-vue-next'

import ConfirmAction from '@/components/layout/ConfirmAction.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { formatEpoch } from '@/lib/control/format'

const q = ref('')
const userId = ref('')
const sort = ref('-updated_epoch_sec')
const { t } = useI18n()
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
const identitiesQuery = computed(() => ({
  q: q.value.trim(),
  user_id: userId.value.trim(),
  limit: 100,
  sort: sort.value,
}))
const hasOAuthFilters = computed(() =>
  q.value.trim() !== '' ||
  userId.value.trim() !== '' ||
  sort.value !== '-updated_epoch_sec',
)
const identities = useAsyncData(() => controlApi.oauthIdentities(identitiesQuery.value))
watch([q, userId, sort], () => identities.refresh())

function resetOAuthFilters() {
  q.value = ''
  userId.value = ''
  sort.value = '-updated_epoch_sec'
}

async function unlinkIdentity(providerUserId: string) {
  await runBusyAction(`unlink:${providerUserId}`, async () => {
    await runWithToast(
      async () => {
        await controlApi.unlinkOAuthIdentity('github', providerUserId)
        await identities.refresh()
      },
      {
        success: t('oauth.toast.unlinked'),
        error: t('oauth.toast.unlinkFailed'),
      },
    )
  })
}
</script>

<template>
  <main class="page-container">
    <PageSection :title="t('route.admin.oauth.title')" :description="t('route.admin.oauth.description')">
      <div class="grid gap-4">
        <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_220px_auto]">
          <SearchToolbar v-model="q" :placeholder="t('oauth.adminSearchPlaceholder')" :loading="identities.loading.value" @refresh="identities.refresh" />
          <Select v-model="sort">
            <SelectTrigger :aria-label="t('oauth.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="-updated_epoch_sec">{{ t('common.updatedRecently') }}</SelectItem>
              <SelectItem value="login">{{ t('oauth.githubAccount') }}</SelectItem>
              <SelectItem value="email">{{ t('common.email') }}</SelectItem>
              <SelectItem value="user_id">{{ t('common.userId') }}</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" :disabled="!hasOAuthFilters" @click="resetOAuthFilters">
            {{ t('common.reset') }}
          </Button>
        </div>
        <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
          <Input v-model="userId" :placeholder="t('common.exactUserId')" :aria-label="t('common.exactUserId')" />
          <p class="text-sm text-muted-foreground sm:text-right">
            {{ t('oauth.total', { total: identities.data.value?.total ?? 0 }) }}
          </p>
        </div>
        <ResponsiveTable
          :items="identities.data.value?.items ?? []"
          :loading="identities.loading.value"
          :error="identities.error.value"
          :empty-title="t('oauth.empty')"
          @retry="identities.refresh"
        >
          <template #head>
            <TableRow>
              <TableHead>Provider</TableHead>
              <TableHead>{{ t('oauth.githubAccount') }}</TableHead>
              <TableHead>{{ t('common.email') }}</TableHead>
              <TableHead>{{ t('common.user') }}</TableHead>
              <TableHead>{{ t('common.updatedAt') }}</TableHead>
              <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
            </TableRow>
          </template>
          <template #rows>
            <TableRow v-for="identity in identities.data.value?.items ?? []" :key="identity.provider_user_id">
              <TableCell><Badge variant="outline">{{ identity.provider }}</Badge></TableCell>
              <TableCell>
                <div class="font-medium">{{ identity.login }}</div>
                <div class="text-xs text-muted-foreground">{{ identity.provider_user_id }}</div>
              </TableCell>
              <TableCell>{{ identity.email }}</TableCell>
              <TableCell>{{ identity.user_id }}</TableCell>
              <TableCell>{{ formatEpoch(identity.updated_epoch_sec) }}</TableCell>
              <TableCell class="text-right">
                <ConfirmAction
                  :title="t('oauth.unlinkTitle')"
                  :description="t('oauth.unlinkDescription', { login: identity.login })"
                  :confirm-text="t('common.unlink')"
                  variant="outline"
                  :icon="Unlink"
                  :disabled="hasBusyAction"
                  :loading="isBusy('unlink', identity.provider_user_id)"
                  @confirm="unlinkIdentity(identity.provider_user_id)"
                >
                  {{ t('common.unlink') }}
                </ConfirmAction>
              </TableCell>
            </TableRow>
          </template>
          <template #cards>
            <div v-for="identity in identities.data.value?.items ?? []" :key="identity.provider_user_id" class="rounded-md border p-4">
              <div class="flex items-start justify-between gap-3">
                <div class="flex min-w-0 items-center gap-2">
                  <Github class="size-4 text-muted-foreground" />
                  <p class="truncate font-medium">{{ identity.login }}</p>
                </div>
                <Badge variant="outline">{{ identity.provider }}</Badge>
              </div>
              <div class="mt-3">
                <InfoRow :label="t('common.providerId')" :value="identity.provider_user_id" />
                <InfoRow :label="t('common.email')" :value="identity.email" />
                <InfoRow :label="t('common.user')" :value="identity.user_id" />
                <InfoRow :label="t('common.createdAt')" :value="formatEpoch(identity.created_epoch_sec)" />
                <InfoRow :label="t('common.updatedAt')" :value="formatEpoch(identity.updated_epoch_sec)" />
              </div>
              <ConfirmAction
                class="mt-3 w-full"
                :title="t('oauth.unlinkTitle')"
                :description="t('oauth.unlinkDescription', { login: identity.login })"
                :confirm-text="t('common.unlink')"
                variant="outline"
                :icon="Unlink"
                :disabled="hasBusyAction"
                :loading="isBusy('unlink', identity.provider_user_id)"
                @confirm="unlinkIdentity(identity.provider_user_id)"
              >
                {{ t('common.unlink') }}
              </ConfirmAction>
            </div>
          </template>
        </ResponsiveTable>
      </div>
    </PageSection>

    <PageSection :title="t('section.admin.oauth.safetyTitle')" :description="t('section.admin.oauth.safetyDescription')">
      <div class="flex items-start gap-3 rounded-md border p-4 text-sm text-muted-foreground">
        <KeyRound class="mt-0.5 size-4 shrink-0 text-primary" />
        <p>{{ t('oauth.unlinkSafety') }}</p>
      </div>
    </PageSection>
  </main>
</template>
