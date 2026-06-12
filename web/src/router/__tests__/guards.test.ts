import { describe, expect, test } from 'bun:test'

import type { AuthSession } from '@/lib/control/types'
import { resolveHomeRedirect, resolveRouteGuard, type GuardRouteTarget } from '../guards'

const activeUserSession: AuthSession = {
  accessToken: 'token',
  userId: 'user_001',
  subject: 'user@example.com',
  role: 'user',
  expireAt: 1_700_000_100,
}

const activeAdminSession: AuthSession = {
  ...activeUserSession,
  role: 'admin',
}

function route(target: Partial<GuardRouteTarget>): GuardRouteTarget {
  return {
    path: target.path ?? '/center',
    fullPath: target.fullPath ?? target.path ?? '/center',
    meta: target.meta ?? {},
  }
}

describe('router guard helpers', () => {
  test('preserves the requested route when an unauthenticated user hits a protected page', () => {
    expect(
      resolveRouteGuard(route({ path: '/center/devices', fullPath: '/center/devices?status=online', meta: { requiresAuth: true } }), null, {
        nowSeconds: 1_700_000_000,
      }),
    ).toEqual({
      path: '/login',
      query: { redirect: '/center/devices?status=online' },
    })
  })

  test('clears expired sessions before redirecting protected routes to login', () => {
    let cleared = false

    expect(
      resolveRouteGuard(
        route({ path: '/center/devices', fullPath: '/center/devices', meta: { requiresAuth: true } }),
        { ...activeUserSession, expireAt: 1_700_000_000 },
        {
          nowSeconds: 1_700_000_000,
          clearSession: () => {
            cleared = true
          },
        },
      ),
    ).toEqual({
      path: '/login',
      query: { redirect: '/center/devices' },
    })
    expect(cleared).toBe(true)
  })

  test('sends non-admin users away from admin routes', () => {
    expect(
      resolveRouteGuard(route({ path: '/admin/users', fullPath: '/admin/users', meta: { requiresAuth: true, requiresAdmin: true } }), activeUserSession, {
        nowSeconds: 1_700_000_000,
      }),
    ).toEqual({ path: '/center' })
  })

  test('sends authenticated users from login to their default workspace', () => {
    expect(resolveRouteGuard(route({ path: '/login', fullPath: '/login' }), activeAdminSession, { nowSeconds: 1_700_000_000 })).toBe('/admin')
    expect(resolveRouteGuard(route({ path: '/login', fullPath: '/login' }), activeUserSession, { nowSeconds: 1_700_000_000 })).toBe('/center')
  })

  test('resolves home by usable session and clears expired sessions', () => {
    let cleared = false

    expect(resolveHomeRedirect(activeAdminSession, { nowSeconds: 1_700_000_000 })).toBe('/admin')
    expect(resolveHomeRedirect(activeUserSession, { nowSeconds: 1_700_000_000 })).toBe('/center')
    expect(
      resolveHomeRedirect(
        { ...activeUserSession, expireAt: 1_700_000_000 },
        {
          nowSeconds: 1_700_000_000,
          clearSession: () => {
            cleared = true
          },
        },
      ),
    ).toBe('/login')
    expect(cleared).toBe(true)
  })
})
