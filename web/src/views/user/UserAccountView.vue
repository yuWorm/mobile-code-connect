<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Github, Loader2, ShieldCheck, Unlink } from 'lucide-vue-next'

import ConfirmAction from '@/components/layout/ConfirmAction.vue'
import InfoRow from '@/components/layout/InfoRow.vue'
import PageSection from '@/components/layout/PageSection.vue'
import ResponsiveTable from '@/components/layout/ResponsiveTable.vue'
import SearchToolbar from '@/components/layout/SearchToolbar.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { TableCell, TableHead, TableRow } from '@/components/ui/table'
import { useAuth } from '@/composables/useAuth'
import { useAsyncData } from '@/composables/useAsyncData'
import { useBusyAction } from '@/composables/useBusyAction'
import { useI18n } from '@/composables/useI18n'
import { runWithToast } from '@/lib/control/action'
import { controlApi } from '@/lib/control/client'
import { formatEpoch } from '@/lib/control/format'
import { formatRoleLabel } from '@/lib/control/labels'
import { buildGithubOAuthStartPath, githubOAuthCallbackUrl } from '@/lib/control/oauth'

const { state } = useAuth()
const q = ref('')
const sort = ref('-updated_epoch_sec')
const saving = ref(false)
const oauthLoading = ref(false)
const passwordMessage = ref('')
const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
const { t } = useI18n()
const passwordForm = reactive({
  current_password: '',
  new_password: '',
})
const identitiesQuery = computed(() => ({
  q: q.value.trim(),
  limit: 100,
  sort: sort.value,
}))
const hasOAuthFilters = computed(() =>
  q.value.trim() !== '' ||
  sort.value !== '-updated_epoch_sec',
)
const hasPasswordForm = computed(() =>
  passwordForm.current_password.trim() !== '' ||
  passwordForm.new_password.trim() !== '',
)
const canUpdatePasswordForm = computed(() =>
  passwordForm.new_password.trim() !== '',
)
const identities = useAsyncData(() => controlApi.oauthIdentities(identitiesQuery.value))
watch([q, sort], () => identities.refresh())

function startGithubOAuth() {
  oauthLoading.value = true
  const redirectUri = githubOAuthCallbackUrl(window.location.href, '/center/account')
  window.location.href = buildGithubOAuthStartPath(redirectUri)
}

function resetOAuthFilters() {
  q.value = ''
  sort.value = '-updated_epoch_sec'
}

function resetPasswordForm() {
  passwordForm.current_password = ''
  passwordForm.new_password = ''
  passwordMessage.value = ''
}

async function updatePassword() {
  if (saving.value || !canUpdatePasswordForm.value) {
    return
  }
  saving.value = true
  passwordMessage.value = ''
  try {
    await runWithToast(
      async () => {
        await controlApi.updatePassword({
          current_password: passwordForm.current_password || null,
          new_password: passwordForm.new_password,
        })
        resetPasswordForm()
        passwordMessage.value = t('account.passwordUpdated')
      },
      {
        success: t('account.passwordUpdated'),
        error: t('account.passwordUpdateFailed'),
      },
    )
  } finally {
    saving.value = false
  }
}

async function unlink(providerUserId: string) {
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
    <PageSection :title="t('section.user.account.infoTitle')" :description="t('section.user.account.infoDescription')">
      <div class="grid gap-3 sm:grid-cols-2">
        <InfoRow :label="t('common.userId')" :value="state.session?.userId" />
        <InfoRow :label="t('common.subject')" :value="state.session?.subject" />
        <InfoRow :label="t('common.role')" :value="formatRoleLabel(state.session?.role, t)" />
        <InfoRow :label="t('common.expiresAt')" :value="state.session?.expireAt" />
      </div>
    </PageSection>

    <PageSection :title="t('section.user.account.passwordTitle')" :description="t('section.user.account.passwordDescription')">
      <form class="grid max-w-xl gap-4" @submit.prevent="updatePassword">
        <div class="grid gap-2">
          <Label for="current-password">{{ t('account.currentPassword') }}</Label>
          <Input id="current-password" v-model="passwordForm.current_password" type="password" autocomplete="current-password" />
        </div>
        <div class="grid gap-2">
          <Label for="new-password">{{ t('account.newPassword') }}</Label>
          <Input id="new-password" v-model="passwordForm.new_password" required type="password" minlength="8" autocomplete="new-password" />
        </div>
        <div class="flex flex-wrap items-center gap-3">
          <Button type="submit" :disabled="saving || !canUpdatePasswordForm">
            <Loader2 v-if="saving" class="animate-spin" />
            {{ t('account.updatePassword') }}
          </Button>
          <Button type="button" variant="outline" :disabled="saving || !hasPasswordForm" @click="resetPasswordForm">
            {{ t('common.reset') }}
          </Button>
          <p v-if="passwordMessage" class="text-sm text-success">{{ passwordMessage }}</p>
        </div>
      </form>
    </PageSection>

    <PageSection :title="t('section.user.account.oauthTitle')" :description="t('section.user.account.oauthDescription')">
      <template #actions>
        <Button variant="outline" :disabled="oauthLoading" @click="startGithubOAuth">
          <Loader2 v-if="oauthLoading" class="animate-spin" />
          <Github v-else class="size-4" />
          {{ t('oauth.linkGithubSameEmail') }}
        </Button>
      </template>
      <div class="mb-4 rounded-md border bg-muted/40 p-3 text-sm text-muted-foreground">
        {{ t('oauth.linkGithubDescription') }}
      </div>
      <div class="mb-4 grid gap-3 lg:grid-cols-[minmax(0,1fr)_220px_auto]">
        <SearchToolbar v-model="q" :placeholder="t('oauth.accountSearchPlaceholder')" :loading="identities.loading.value" @refresh="identities.refresh" />
        <Select v-model="sort">
          <SelectTrigger :aria-label="t('oauth.sortLabel')"><SelectValue :placeholder="t('common.sort')" /></SelectTrigger>
          <SelectContent>
            <SelectItem value="-updated_epoch_sec">{{ t('common.updatedRecently') }}</SelectItem>
            <SelectItem value="login">{{ t('oauth.githubAccount') }}</SelectItem>
            <SelectItem value="email">{{ t('common.email') }}</SelectItem>
            <SelectItem value="provider_user_id">{{ t('common.providerId') }}</SelectItem>
          </SelectContent>
        </Select>
        <Button variant="outline" :disabled="!hasOAuthFilters" @click="resetOAuthFilters">
          {{ t('common.reset') }}
        </Button>
      </div>
      <p class="mb-4 text-sm text-muted-foreground sm:text-right">
        {{ t('oauth.total', { total: identities.data.value?.total ?? 0 }) }}
      </p>
      <ResponsiveTable :items="identities.data.value?.items ?? []" :loading="identities.loading.value" :error="identities.error.value" :empty-title="t('oauth.empty')" @retry="identities.refresh">
        <template #head>
          <TableRow>
            <TableHead>Provider</TableHead>
            <TableHead>{{ t('common.account') }}</TableHead>
            <TableHead>{{ t('common.email') }}</TableHead>
            <TableHead>{{ t('common.updatedAt') }}</TableHead>
            <TableHead class="text-right">{{ t('common.actions') }}</TableHead>
          </TableRow>
        </template>
        <template #rows>
          <TableRow v-for="identity in identities.data.value?.items ?? []" :key="identity.provider_user_id">
            <TableCell><Badge variant="outline">{{ identity.provider }}</Badge></TableCell>
            <TableCell>{{ identity.login }}<div class="text-xs text-muted-foreground">{{ identity.provider_user_id }}</div></TableCell>
            <TableCell>{{ identity.email }}</TableCell>
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
                @confirm="unlink(identity.provider_user_id)"
              >
                {{ t('common.unlink') }}
              </ConfirmAction>
            </TableCell>
          </TableRow>
        </template>
        <template #cards>
          <div v-for="identity in identities.data.value?.items ?? []" :key="identity.provider_user_id" class="rounded-md border p-4">
            <div class="flex items-center gap-2">
              <Github class="size-4 text-muted-foreground" />
              <p class="font-medium">{{ identity.login }}</p>
              <Badge variant="outline">{{ identity.provider }}</Badge>
            </div>
            <div class="mt-3">
              <InfoRow :label="t('common.providerId')" :value="identity.provider_user_id" />
              <InfoRow :label="t('common.email')" :value="identity.email" />
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
              @confirm="unlink(identity.provider_user_id)"
            >
              {{ t('common.unlink') }}
            </ConfirmAction>
          </div>
        </template>
      </ResponsiveTable>
    </PageSection>

    <PageSection :title="t('section.user.account.safetyTitle')" :description="t('section.user.account.safetyDescription')">
      <div class="flex items-start gap-3 rounded-md border p-4 text-sm text-muted-foreground">
        <ShieldCheck class="mt-0.5 size-4 shrink-0 text-primary" />
        <p>{{ t('oauth.accountUnlinkSafety') }}</p>
      </div>
    </PageSection>
  </main>
</template>
