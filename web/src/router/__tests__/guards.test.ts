import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

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

  test('preserves server-auth approval routes when login is required', () => {
    expect(
      resolveRouteGuard(
        route({
          path: '/server-auth/browser/approve',
          fullPath: '/server-auth/browser/approve?session_id=sess_001',
          meta: { requiresAuth: true },
        }),
        null,
        { nowSeconds: 1_700_000_000 },
      ),
    ).toEqual({
      path: '/login',
      query: { redirect: '/server-auth/browser/approve?session_id=sess_001' },
    })

    expect(
      resolveRouteGuard(
        route({
          path: '/server-auth/device',
          fullPath: '/server-auth/device?user_code=ABCD-EFGH',
          meta: { requiresAuth: true },
        }),
        null,
        { nowSeconds: 1_700_000_000 },
      ),
    ).toEqual({
      path: '/login',
      query: { redirect: '/server-auth/device?user_code=ABCD-EFGH' },
    })
  })

  test('registers protected server-auth approval routes', () => {
    const source = readFileSync(new URL('../index.ts', import.meta.url), 'utf8')

    expect(source).toContain("path: '/server-auth/browser/approve'")
    expect(source).toContain("name: 'server-auth-browser-approve'")
    expect(source).toContain("component: () => import('@/views/ServerAuthBrowserApproveView.vue')")
    expect(source).toContain("meta: { requiresAuth: true, title: 'Server Login Approval'")
    expect(source).toContain("path: '/server-auth/device'")
    expect(source).toContain("name: 'server-auth-device'")
    expect(source).toContain("component: () => import('@/views/ServerAuthDeviceApproveView.vue')")
    expect(source).toContain("meta: { requiresAuth: true, title: 'Device Code Approval'")
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
