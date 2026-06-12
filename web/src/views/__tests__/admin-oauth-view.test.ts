import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

describe('admin oauth identities view', () => {
  test('registers an admin route and navigation item', () => {
    const router = readFileSync(new URL('../../router/index.ts', import.meta.url), 'utf8')
    const nav = readFileSync(new URL('../../components/layout/nav.ts', import.meta.url), 'utf8')

    expect(router).toContain("path: 'oauth'")
    expect(router).toContain("name: 'admin-oauth'")
    expect(router).toContain("component: () => import('@/views/admin/AdminOAuthView.vue')")
    expect(nav).toContain("to: '/admin/oauth'")
    expect(nav).toContain("label: 'OAuth'")
  })

  test('lists and unlinks oauth identities with search', () => {
    const source = readFileSync(new URL('../admin/AdminOAuthView.vue', import.meta.url), 'utf8')

    expect(source).toContain("const q = ref('')")
    expect(source).toContain("const userId = ref('')")
    expect(source).toContain("const sort = ref('-updated_epoch_sec')")
    expect(source).toContain('const identitiesQuery = computed(() => ({')
    expect(source).toContain('q: q.value.trim()')
    expect(source).toContain('user_id: userId.value.trim()')
    expect(source).toContain('sort: sort.value')
    expect(source).toContain('controlApi.oauthIdentities(identitiesQuery.value)')
    expect(source).toContain('watch([q, userId, sort], () => identities.refresh())')
    expect(source).toContain('controlApi.unlinkOAuthIdentity')
    expect(source).toContain('<SearchToolbar v-model="q" :placeholder="t(\'oauth.adminSearchPlaceholder\')"')
    expect(source).toContain('<Input v-model="userId"')
    expect(source).toContain('<Input v-model="userId" :placeholder="t(\'common.exactUserId\')" :aria-label="t(\'common.exactUserId\')" />')
    expect(source).toContain('<Select v-model="sort">')
    expect(source).toContain('<SelectTrigger :aria-label="t(\'oauth.sortLabel\')"><SelectValue :placeholder="t(\'common.sort\')" /></SelectTrigger>')
    expect(source).toContain('<SelectItem value="login">{{ t(\'oauth.githubAccount\') }}</SelectItem>')
    expect(source).toContain('<SelectItem value="email">{{ t(\'common.email\') }}</SelectItem>')
    expect(source).toContain("t('oauth.total'")
    expect(source).toContain('ConfirmAction')
  })

  test('can reset admin oauth identity filters back to defaults', () => {
    const source = readFileSync(new URL('../admin/AdminOAuthView.vue', import.meta.url), 'utf8')

    expect(source).toContain('const hasOAuthFilters = computed(() =>')
    expect(source).toContain("sort.value !== '-updated_epoch_sec'")
    expect(source).toContain('function resetOAuthFilters()')
    expect(source).toContain("q.value = ''")
    expect(source).toContain("userId.value = ''")
    expect(source).toContain("sort.value = '-updated_epoch_sec'")
    expect(source).toContain(':disabled="!hasOAuthFilters"')
    expect(source).toContain('@click="resetOAuthFilters"')
  })

  test('admin oauth visible copy is localized', () => {
    const source = readFileSync(new URL('../admin/AdminOAuthView.vue', import.meta.url), 'utf8')

    expect(source).toContain("success: t('oauth.toast.unlinked')")
    expect(source).toContain("error: t('oauth.toast.unlinkFailed')")
    expect(source).toContain(":empty-title=\"t('oauth.empty')\"")
    expect(source).toContain("<TableHead>{{ t('oauth.githubAccount') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.email') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.user') }}</TableHead>")
    expect(source).toContain("<TableHead>{{ t('common.updatedAt') }}</TableHead>")
    expect(source).toContain("<TableHead class=\"text-right\">{{ t('common.actions') }}</TableHead>")
    expect(source).toContain(":title=\"t('oauth.unlinkTitle')\"")
    expect(source).toContain(":description=\"t('oauth.unlinkDescription', { login: identity.login })\"")
    expect(source).toContain(":confirm-text=\"t('common.unlink')\"")
    expect(source).toContain("{{ t('common.unlink') }}")
    expect(source).toContain(":label=\"t('common.email')\"")
    expect(source).toContain(":label=\"t('common.user')\"")
    expect(source).toContain(":label=\"t('common.createdAt')\"")
    expect(source).toContain(":label=\"t('common.updatedAt')\"")
    expect(source).toContain("{{ t('oauth.unlinkSafety') }}")
    expect(source).not.toContain('OAuth 身份已解绑')
    expect(source).not.toContain('解绑 OAuth 身份')
    expect(source).not.toContain('暂无 OAuth 身份')
    expect(source).not.toContain('如果解绑会导致')
  })
})
