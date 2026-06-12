import type { RouteLocationRaw, RouteMeta } from 'vue-router'

import { isStoredSessionUsable, redirectForRole } from '@/lib/control/auth'
import type { AuthSession } from '@/lib/control/types'

export interface GuardRouteTarget {
  path: string
  fullPath: string
  meta: RouteMeta
}

interface GuardOptions {
  clearSession?: () => void
  nowSeconds?: number
}

function usableSession(session: AuthSession | null, options: GuardOptions) {
  if (isStoredSessionUsable(session, options.nowSeconds)) {
    return session
  }
  if (session) {
    options.clearSession?.()
  }
  return null
}

export function resolveHomeRedirect(session: AuthSession | null, options: GuardOptions = {}): string {
  const activeSession = usableSession(session, options)
  return activeSession ? redirectForRole(activeSession.role) : '/login'
}

export function resolveRouteGuard(
  to: GuardRouteTarget,
  session: AuthSession | null,
  options: GuardOptions = {},
): true | RouteLocationRaw {
  const activeSession = usableSession(session, options)

  if (to.meta.requiresAuth && !activeSession) {
    return { path: '/login', query: { redirect: to.fullPath } }
  }

  if (to.meta.requiresAdmin && activeSession?.role !== 'admin') {
    return { path: '/center' }
  }

  if (to.path === '/login' && activeSession) {
    return redirectForRole(activeSession.role)
  }

  return true
}
