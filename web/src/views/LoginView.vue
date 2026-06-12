<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { Github, Languages, Loader2, LockKeyhole, Network, ShieldCheck } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useAuth } from '@/composables/useAuth'
import { useI18n } from '@/composables/useI18n'
import { buildGithubOAuthStartPath, githubOAuthCallbackUrl } from '@/lib/control/oauth'

const { state, login, register } = useAuth()
const { locale, locales, setLocale, t } = useI18n()
const mode = ref('login')
const oauthLoading = ref(false)
const form = reactive({
  email: '',
  password: '',
  displayName: '',
})

const title = computed(() => (mode.value === 'login' ? t('auth.loginTitle') : t('auth.registerTitle')))
const hasLoginForm = computed(() =>
  form.email.trim() !== '' ||
  form.password.trim() !== '' ||
  form.displayName.trim() !== '',
)

watch(mode, () => {
  form.password = ''
  form.displayName = ''
})

function resetLoginForm() {
  form.email = ''
  form.password = ''
  form.displayName = ''
}

function startGithubOAuth() {
  oauthLoading.value = true
  const redirectUri = githubOAuthCallbackUrl(window.location.href)
  window.location.href = buildGithubOAuthStartPath(redirectUri)
}

async function submit() {
  if (mode.value === 'login') {
    await login({ email: form.email, password: form.password })
    return
  }
  await register({
    email: form.email,
    password: form.password,
    display_name: form.displayName || form.email.split('@')[0],
  })
}
</script>

<template>
  <main class="relative flex min-h-screen flex-col overflow-x-hidden bg-background lg:grid lg:grid-cols-2">
    <header class="absolute inset-x-0 top-0 z-20 flex items-center px-5 py-5 sm:px-8 lg:px-12 lg:py-8">
      <div class="flex min-w-0 max-w-[calc(100%-8rem)] items-center gap-3 pr-4">
        <div class="flex size-10 shrink-0 items-center justify-center rounded-md bg-primary text-sm font-semibold text-primary-foreground shadow-sm">
          {{ t('app.logo') }}
        </div>
        <div class="min-w-0">
          <p class="max-w-[8.5rem] truncate sm:max-w-none text-sm font-semibold sm:text-base">
            {{ t('app.name') }}
          </p>
          <p class="hidden text-xs text-muted-foreground sm:block">{{ t('app.console') }}</p>
        </div>
      </div>

      <div class="absolute right-5 top-5 sm:right-8 lg:right-12 lg:top-8">
        <Select :model-value="locale" @update:model-value="setLocale(String($event))">
          <SelectTrigger class="h-9 w-[112px] sm:w-[132px]" :aria-label="t('shell.language')">
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
    </header>

    <section class="login-brand-panel order-2 flex min-h-[56vh] items-center justify-center overflow-hidden border-t px-5 py-14 text-center sm:px-8 lg:order-1 lg:min-h-screen lg:border-r lg:border-t-0 lg:px-16">
      <div class="mx-auto flex w-full max-w-xl flex-col items-center justify-center pt-8 lg:pt-20">
        <p class="text-xs font-semibold uppercase tracking-[0.24em] text-primary">
          {{ t('auth.heroEyebrow') }}
        </p>
        <h1 class="mt-5 max-w-xl text-3xl font-semibold leading-tight tracking-normal text-foreground sm:text-4xl">
          {{ t('auth.heroSlogan') }}
        </h1>
        <p class="mt-5 max-w-lg text-base leading-7 text-muted-foreground">
          {{ t('auth.heroDescription') }}
        </p>

        <div aria-hidden="true" class="relative mt-10 h-64 w-full max-w-md">
          <div class="absolute left-1/2 top-1/2 h-px w-[82%] -translate-x-1/2 bg-primary/25" />
          <div class="absolute left-1/2 top-10 h-44 w-px -translate-x-1/2 bg-primary/20" />
          <div class="absolute left-[18%] top-7 flex size-16 items-center justify-center rounded-md border bg-background/85 text-primary shadow-sm backdrop-blur">
            <Network class="size-7" />
          </div>
          <div class="absolute right-[18%] top-7 flex size-16 items-center justify-center rounded-md border bg-background/85 text-primary shadow-sm backdrop-blur">
            <ShieldCheck class="size-7" />
          </div>
          <div class="absolute bottom-4 left-1/2 flex size-16 -translate-x-1/2 items-center justify-center rounded-md border bg-background/85 text-primary shadow-sm backdrop-blur">
            <LockKeyhole class="size-7" />
          </div>
          <div class="absolute left-1/2 top-1/2 flex size-24 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-lg border border-primary/25 bg-background text-xl font-semibold text-primary shadow-md">
            {{ t('app.logo') }}
          </div>
        </div>

        <p class="mt-8 text-sm text-muted-foreground">{{ t('auth.heroFooter') }}</p>
      </div>
    </section>

    <section class="order-1 flex min-h-screen items-center justify-center px-5 pb-10 pt-24 sm:px-8 lg:order-2 lg:px-16 lg:pt-20">
      <div class="min-w-0 w-full max-w-[20rem] sm:max-w-[28rem]">
        <div class="mb-8">
          <p class="text-xs font-semibold uppercase tracking-[0.22em] text-primary">
            {{ t('auth.formEyebrow') }}
          </p>
          <h2 class="mt-3 text-2xl font-semibold leading-tight tracking-normal text-foreground sm:text-3xl">
            {{ title }}
          </h2>
          <p class="mt-3 text-sm leading-6 text-muted-foreground">{{ t('auth.subtitle') }}</p>
        </div>

        <Tabs v-model="mode" class="w-full">
          <TabsList class="grid w-full grid-cols-2">
            <TabsTrigger value="login">{{ t('auth.login') }}</TabsTrigger>
            <TabsTrigger value="register">{{ t('auth.register') }}</TabsTrigger>
          </TabsList>
          <TabsContent value="login" />
          <TabsContent value="register" />
        </Tabs>

        <form class="mt-6 grid gap-4" @submit.prevent="submit">
          <div class="grid gap-2">
            <Label for="email">{{ t('auth.email') }}</Label>
            <Input id="email" v-model="form.email" required type="email" autocomplete="email" />
          </div>
          <div v-if="mode === 'register'" class="grid gap-2">
            <Label for="displayName">{{ t('auth.displayName') }}</Label>
            <Input id="displayName" v-model="form.displayName" autocomplete="name" />
          </div>
          <div class="grid gap-2">
            <Label for="password">{{ t('auth.password') }}</Label>
            <Input
              id="password"
              v-model="form.password"
              required
              type="password"
              :autocomplete="mode === 'login' ? 'current-password' : 'new-password'"
              minlength="8"
            />
          </div>

          <p v-if="state.error" class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {{ state.error }}
          </p>

          <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
            <Button type="submit" :disabled="state.loading">
              <Loader2 v-if="state.loading" class="animate-spin" />
              {{ mode === 'login' ? t('auth.login') : t('auth.registerAndEnter') }}
            </Button>
            <Button type="button" variant="outline" :disabled="state.loading || !hasLoginForm" @click="resetLoginForm">
              {{ t('auth.clear') }}
            </Button>
          </div>
          <div class="grid gap-3">
            <div class="flex items-center gap-3">
              <div class="h-px flex-1 bg-border" />
              <span class="text-xs text-muted-foreground">OAuth</span>
              <div class="h-px flex-1 bg-border" />
            </div>
            <Button
              type="button"
              variant="outline"
              class="w-full"
              :disabled="state.loading || oauthLoading"
              @click="startGithubOAuth"
            >
              <Loader2 v-if="oauthLoading" class="animate-spin" />
              <Github v-else class="size-4" />
              {{ t('auth.githubLogin') }}
            </Button>
          </div>

          <RouterLink class="text-center text-sm text-muted-foreground hover:text-foreground" to="/">
            {{ t('auth.backHome') }}
          </RouterLink>
        </form>
      </div>
    </section>
  </main>
</template>
