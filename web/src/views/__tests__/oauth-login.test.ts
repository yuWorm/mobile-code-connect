import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('OAuth login views', () => {
  test('login page links GitHub OAuth through the start helper', () => {
    const source = readFileSync(new URL('../LoginView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { RouterLink, useRoute } from 'vue-router'")
    expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(source).toContain('const { locale, locales, setLocale, t } = useI18n()')
    expect(source).toContain('const route = useRoute()')
    expect(source).toContain('startGithubOAuth')
    expect(source).toContain('buildGithubOAuthStartPath')
    expect(source).toContain('githubOAuthCallbackUrl')
    expect(source).toContain("const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : undefined")
    expect(source).toContain('const redirectUri = githubOAuthCallbackUrl(window.location.href, redirect)')
    expect(source).toContain("const title = computed(() => (mode.value === 'login' ? t('auth.loginTitle') : t('auth.registerTitle')))")
    expect(source).toContain("{{ mode === 'login' ? t('auth.login') : t('auth.registerAndEnter') }}")
    expect(source).toContain('<Select :model-value="locale" @update:model-value="setLocale(String($event))">')
    expect(source).toContain(':aria-label="t(\'shell.language\')"')
  })

  test('login page prevents repeated GitHub OAuth redirects', () => {
    const source = readFileSync(new URL('../LoginView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const oauthLoading = ref(false)')
    expect(source).toContain('oauthLoading.value = true')
    expect(source).toContain(':disabled="state.loading || oauthLoading"')
    expect(source).toContain('<Loader2 v-if="oauthLoading" class="animate-spin" />')
    expect(source).toContain('<Github v-else class="size-4" />')
  })

  test('login page clears sensitive form fields when switching modes', () => {
    const source = readFileSync(new URL('../LoginView.vue', import.meta.url), 'utf8')

    expect(source).toContain("watch(mode, () => {")
    expect(source).toContain("form.password = ''")
    expect(source).toContain("form.displayName = ''")
    expect(source).toContain('const hasLoginForm = computed(() =>')
    expect(source).toContain('function resetLoginForm()')
    expect(source).toContain("form.email = ''")
    expect(source).toContain(':autocomplete="mode === \'login\' ? \'current-password\' : \'new-password\'"')
    expect(source).toContain(':disabled="state.loading || !hasLoginForm"')
    expect(source).toContain('@click="resetLoginForm"')
  })

  test('callback page exchanges code and state into an auth session', () => {
    const source = readFileSync(new URL('../OAuthGithubCallbackView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(source).toContain('const { locale, locales, setLocale, t } = useI18n()')
    expect(source).toContain('githubOAuthCallback')
    expect(source).toContain('safeRedirectTarget')
    expect(source).toContain('setSession(sessionFromAuthResponse(response))')
    expect(source).toContain('router.replace(safeRedirectTarget(route.query.redirect, state.session?.role))')
    expect(source).toContain("error.value = t('oauthCallback.missingCode')")
    expect(source).toContain("unauthorized: t('oauthCallback.invalidCredential')")
    expect(source).toContain('<Select :model-value="locale" @update:model-value="setLocale(String($event))">')
  })

  test('email auth returns to safe redirect targets after login and register', () => {
    const source = readFileSync(new URL('../../composables/useAuth.ts', import.meta.url), 'utf8')

    expect(source).toContain("import { useRoute, useRouter } from 'vue-router'")
    expect(source).toContain('safeRedirectTarget')
    expect(source.match(/router\.replace\(safeRedirectTarget\(route\.query\.redirect, state\.session\?\.role\)\)/g)?.length).toBe(2)
  })

  test('account page offers GitHub same-email linking through OAuth', () => {
    const source = readFileSync(new URL('../user/UserAccountView.vue', import.meta.url), 'utf8')

    expect(source).toContain('startGithubOAuth')
    expect(source).toContain("githubOAuthCallbackUrl(window.location.href, '/center/account')")
    expect(source).toContain("{{ t('oauth.linkGithubSameEmail') }}")
  })

  test('account page shows the current role with localized labels', () => {
    const source = readFileSync(new URL('../user/UserAccountView.vue', import.meta.url), 'utf8')

    expect(source).toContain("import { formatRoleLabel } from '@/lib/control/labels'")
    expect(source).toContain('<InfoRow :label="t(\'common.role\')" :value="formatRoleLabel(state.session?.role, t)" />')
    expect(source).not.toContain('<InfoRow label="角色" :value="state.session?.role" />')
  })

  test('account password form can be reset and clears sensitive fields after success', () => {
    const source = readFileSync(new URL('../user/UserAccountView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasPasswordForm = computed(() =>')
    expect(source).toContain("passwordForm.current_password.trim() !== ''")
    expect(source).toContain("passwordForm.new_password.trim() !== ''")
    expect(source).toContain('const canUpdatePasswordForm = computed(() =>')
    expect(source).toContain('function resetPasswordForm()')
    expect(source).toContain("passwordForm.current_password = ''")
    expect(source).toContain("passwordForm.new_password = ''")
    expect(source).toContain('resetPasswordForm()')
    expect(source).toContain('for="current-password"')
    expect(source).toContain('id="current-password"')
    expect(source).toContain('for="new-password"')
    expect(source).toContain('id="new-password"')
    expect(source).toContain(`async function updatePassword() {
  if (saving.value || !canUpdatePasswordForm.value) {
    return
  }`)
    expect(source).toContain('<Button type="submit" :disabled="saving || !canUpdatePasswordForm">')
    expect(source).toContain(':disabled="saving || !hasPasswordForm"')
    expect(source).toContain('@click="resetPasswordForm"')
  })

  test('account page prevents repeated GitHub OAuth linking redirects', () => {
    const source = readFileSync(new URL('../user/UserAccountView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const oauthLoading = ref(false)')
    expect(source).toContain('oauthLoading.value = true')
    expect(source).toContain(':disabled="oauthLoading"')
    expect(source).toContain('<Loader2 v-if="oauthLoading" class="animate-spin" />')
    expect(source).toContain('<Github v-else class="size-4" />')
  })

  test('account page filters and sorts linked oauth identities', () => {
    const source = readFileSync(new URL('../user/UserAccountView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const q = ref('')")
    expect(source).toContain("const sort = ref('-updated_epoch_sec')")
    expect(source).toContain('const identitiesQuery = computed(() => ({')
    expect(source).toContain('q: q.value.trim()')
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('controlApi.oauthIdentities(identitiesQuery.value)')
    expect(source).toContain('watch([q, sort], () => identities.refresh())')
    expect(source).toContain('<SearchToolbar v-model="q"')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'oauth.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem value="login">{{ t(\'oauth.githubAccount\') }}</SelectItem>')
    expect(source).toContain("t('oauth.total'")
  })

  test('account page can reset linked oauth identity filters', () => {
    const source = readFileSync(new URL('../user/UserAccountView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasOAuthFilters = computed(() =>')
    expect(source).toContain("sort.value !== '-updated_epoch_sec'")
    expect(source).toContain('function resetOAuthFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("sort.value = '-updated_epoch_sec'")
    expect(source).toContain(':disabled="!hasOAuthFilters"')
    expect(source).toContain('@click="resetOAuthFilters"')
  })

  test('account page visible copy is localized', () => {
    const source = readFileSync(new URL('../user/UserAccountView.vue', import.meta.url), 'utf8')

    expect(source).toContain("passwordMessage.value = t('account.passwordUpdated')")
    expect(source).toContain("success: t('account.passwordUpdated')")
    expect(source).toContain("error: t('account.passwordUpdateFailed')")
    expect(source).toContain("success: t('oauth.toast.unlinked')")
    expect(source).toContain("error: t('oauth.toast.unlinkFailed')")
    expect(source).toContain("<InfoRow :label=\"t('common.userId')\"")
    expect(source).toContain("<InfoRow :label=\"t('common.expiresAt')\"")
    expect(source).toContain("<Label for=\"current-password\">{{ t('account.currentPassword') }}</Label>")
    expect(source).toContain("<Label for=\"new-password\">{{ t('account.newPassword') }}</Label>")
    expect(source).toContain("{{ t('account.updatePassword') }}")
    expect(source).toContain("{{ t('common.reset') }}")
    expect(source).toContain("{{ t('oauth.linkGithubDescription') }}")
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'oauth.accountSearchPlaceholder\')"')
    expect(source).toContain(":empty-title=\"t('oauth.empty')\"")
    expect(source).toContain("<TableHead>{{ t('common.account') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.email') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.updatedAt') }}</TableHead>")
    expect(source).toContain(":title=\"t('oauth.unlinkTitle')\"")
    expect(source).toContain(":description=\"t('oauth.unlinkDescription', { login: identity.login })\"")
    expect(source).toContain(":confirm-text=\"t('common.unlink')\"")
    expect(source).toContain("{{ t('oauth.accountUnlinkSafety') }}")
    expect(source).not.toContain('密码已更新')
    expect(source).not.toContain('当前密码')
    expect(source).not.toContain('使用 GitHub 同邮箱关联')
    expect(source).not.toContain('暂无 OAuth 身份')
  })
})
