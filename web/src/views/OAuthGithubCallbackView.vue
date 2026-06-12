<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Github, Languages, Loader2 } from 'lucide-vue-next'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { useAuth } from '@/composables/useAuth'
import { useI18n } from '@/composables/useI18n'
import { controlApiErrorMessage } from '@/lib/control/api'
import { controlApi } from '@/lib/control/client'
import { safeRedirectTarget, sessionFromAuthResponse } from '@/lib/control/auth'

const route = useRoute()
const router = useRouter()
const { state, setSession } = useAuth()
const { locale, locales, setLocale, t } = useI18n()
const error = ref('')

onMounted(exchangeGithubOAuth)

async function exchangeGithubOAuth() {
  const code = String(route.query.code ?? '')
  const oauthState = String(route.query.state ?? '')
  if (!code || !oauthState) {
    error.value = t('oauthCallback.missingCode')
    return
  }

  try {
    const response = await controlApi.githubOAuthCallback({ code, state: oauthState })
    setSession(sessionFromAuthResponse(response))
    await router.replace(safeRedirectTarget(route.query.redirect, state.session?.role))
  } catch (caught) {
    error.value = controlApiErrorMessage(caught, {
      unauthorized: t('oauthCallback.invalidCredential'),
      forbidden: t('oauthCallback.forbidden'),
      fallback: t('oauthCallback.failed'),
    })
  }
}
</script>

<template>
  <main class="flex min-h-screen items-center justify-center bg-background px-4 py-8">
    <div class="absolute right-4 top-4">
      <Select :model-value="locale" @update:model-value="setLocale(String($event))">
        <SelectTrigger class="w-[132px]" :aria-label="t('shell.language')">
          <Languages class="size-4" />
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="locale in locales" :key="locale.value" :value="locale.value">
            {{ locale.label }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
    <Card class="w-full max-w-md">
      <CardHeader>
        <div class="mb-2 flex size-10 items-center justify-center rounded-md bg-primary text-primary-foreground">
          <Github class="size-5" />
        </div>
        <CardTitle>{{ t('oauthCallback.title') }}</CardTitle>
        <CardDescription>{{ t('oauthCallback.description') }}</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-4">
        <div v-if="!error" class="flex items-center gap-2 rounded-md border p-3 text-sm text-muted-foreground">
          <Loader2 class="size-4 animate-spin" />
          {{ t('oauthCallback.loading') }}
        </div>
        <div v-else class="grid gap-3">
          <p class="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
            {{ error }}
          </p>
          <Button variant="outline" @click="router.replace('/login')">{{ t('oauthCallback.backLogin') }}</Button>
        </div>
      </CardContent>
    </Card>
  </main>
</template>
