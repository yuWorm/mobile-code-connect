<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { Languages, LogOut, Menu, Moon, ShieldCheck, Sun, Users, X } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { useAuth } from '@/composables/useAuth'
import { useI18n } from '@/composables/useI18n'
import { useTheme } from '@/composables/useTheme'
import { formatRoleLabel } from '@/lib/control/labels'
import type { I18nKey } from '@/lib/i18n/messages'
import { adminNavItems, centerNavItems } from './nav'

const route = useRoute()
const { state, isAdmin, logout } = useAuth()
const { locale, locales, setLocale, t } = useI18n()
const { isDark, toggleTheme } = useTheme()
const mobileOpen = ref(false)

const isAdminRoute = computed(() => route.path.startsWith('/admin'))
const navItems = computed(() => (isAdminRoute.value ? adminNavItems : centerNavItems))
const workspaceLabel = computed(() => (isAdminRoute.value ? t('shell.workspace.admin') : t('shell.workspace.center')))
const canSwitchAdmin = computed(() => isAdmin.value && !isAdminRoute.value)
const routeTitle = computed(() =>
  typeof route.meta.titleKey === 'string'
    ? t(route.meta.titleKey as I18nKey)
    : String(route.meta.title ?? workspaceLabel.value),
)
const routeDescription = computed(() =>
  typeof route.meta.descriptionKey === 'string'
    ? t(route.meta.descriptionKey as I18nKey)
    : String(route.meta.description ?? t('shell.descriptionFallback')),
)
</script>

<template>
  <TooltipProvider>
    <div class="min-h-screen bg-background">
      <aside
        class="fixed inset-y-0 left-0 z-40 hidden w-64 border-r bg-card/95 backdrop-blur lg:flex lg:flex-col"
      >
        <div class="flex h-16 items-center gap-3 border-b px-5">
          <div class="flex size-9 items-center justify-center rounded-md bg-primary text-primary-foreground">
            {{ t('app.logo') }}
          </div>
          <div class="min-w-0">
            <p class="truncate text-sm font-semibold">{{ t('app.name') }}</p>
            <p class="truncate text-xs text-muted-foreground">{{ workspaceLabel }}</p>
          </div>
        </div>
        <nav class="flex-1 space-y-1 p-3">
          <RouterLink
            v-for="item in navItems"
            :key="item.to"
            :to="item.to"
            :class="[
              'flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground',
              route.path === item.to ? 'bg-accent text-accent-foreground' : '',
            ]"
          >
            <component :is="item.icon" class="size-4" />
            {{ t(item.labelKey) }}
          </RouterLink>
        </nav>
        <div class="border-t p-3">
          <RouterLink
            v-if="canSwitchAdmin"
            to="/admin"
            class="mb-2 flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
          >
            <ShieldCheck class="size-4" />
            {{ t('shell.switchAdmin') }}
          </RouterLink>
          <RouterLink
            v-else-if="isAdminRoute"
            to="/center"
            class="mb-2 flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
          >
            <Users class="size-4" />
            {{ t('shell.switchCenter') }}
          </RouterLink>
          <div class="rounded-md bg-muted p-3 text-xs text-muted-foreground">
            <p class="truncate font-medium text-foreground">{{ state.session?.subject }}</p>
            <p class="mt-1">{{ t('shell.rolePrefix', { role: formatRoleLabel(state.session?.role, t) }) }}</p>
          </div>
        </div>
      </aside>

      <div class="lg:pl-64">
        <header
          class="sticky top-0 z-30 flex h-16 items-center justify-between border-b bg-background/90 px-4 backdrop-blur sm:px-6 lg:px-8"
        >
          <div class="flex min-w-0 items-center gap-3">
            <Button variant="ghost" size="icon" class="lg:hidden" :aria-label="t('shell.openNav')" @click="mobileOpen = true">
              <Menu class="size-5" />
              <span class="sr-only">{{ t('shell.openNav') }}</span>
            </Button>
            <div class="min-w-0">
              <p class="truncate text-sm font-semibold">{{ routeTitle }}</p>
              <p class="hidden truncate text-xs text-muted-foreground sm:block">
                {{ routeDescription }}
              </p>
            </div>
          </div>
          <div class="flex items-center gap-2">
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
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="icon" :aria-label="t('shell.toggleTheme')" @click="toggleTheme">
                  <Sun v-if="isDark" class="size-4" />
                  <Moon v-else class="size-4" />
                  <span class="sr-only">{{ t('shell.toggleTheme') }}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>{{ t('shell.toggleTheme') }}</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="icon" :aria-label="t('shell.logout')" @click="logout">
                  <LogOut class="size-4" />
                  <span class="sr-only">{{ t('shell.logout') }}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>{{ t('shell.logout') }}</TooltipContent>
            </Tooltip>
          </div>
        </header>

        <RouterView />
      </div>

      <div v-if="mobileOpen" class="fixed inset-0 z-50 lg:hidden">
        <div class="absolute inset-0 bg-black/50" @click="mobileOpen = false" />
        <div
          class="absolute inset-y-0 left-0 flex w-[min(88vw,20rem)] flex-col border-r bg-card shadow-xl"
        >
          <div class="flex h-16 items-center justify-between border-b px-4">
            <div>
              <p class="text-sm font-semibold">{{ t('app.name') }}</p>
              <p class="text-xs text-muted-foreground">{{ workspaceLabel }}</p>
            </div>
            <Button variant="ghost" size="icon" :aria-label="t('shell.closeNav')" @click="mobileOpen = false">
              <X class="size-5" />
              <span class="sr-only">{{ t('shell.closeNav') }}</span>
            </Button>
          </div>
          <nav class="flex-1 space-y-1 p-3">
            <RouterLink
              v-for="item in navItems"
              :key="item.to"
              :to="item.to"
              :class="[
                'flex items-center gap-3 rounded-md px-3 py-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground',
                route.path === item.to ? 'bg-accent text-accent-foreground' : '',
              ]"
              @click="mobileOpen = false"
            >
              <component :is="item.icon" class="size-4" />
              {{ t(item.labelKey) }}
            </RouterLink>
          </nav>
        </div>
      </div>
    </div>
  </TooltipProvider>
</template>
