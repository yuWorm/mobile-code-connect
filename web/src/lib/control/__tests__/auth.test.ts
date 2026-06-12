import { describe, expect, test } from 'bun:test'

import {
  decodeControlClaims,
  isSessionExpired,
  isStoredSessionUsable,
  redirectForRole,
  safeRedirectTarget,
  sessionFromAuthResponse,
} from '../auth'
import type { AuthSession } from '../types'

function tokenFor(payload: unknown) {
  const encoded = btoa(JSON.stringify(payload))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')
  return `${encoded}.signature`
}

describe('decodeControlClaims', () => {
  test('decodes role and subject from a signed control token payload', () => {
    const token = tokenFor({
      user_id: 'user_001',
      subject: 'root@example.com',
      role: 'admin',
      exp: 4_102_444_800,
    })

    expect(decodeControlClaims(token)).toEqual({
      user_id: 'user_001',
      subject: 'root@example.com',
      role: 'admin',
      exp: 4_102_444_800,
    })
  })
})

describe('sessionFromAuthResponse', () => {
  test('uses token claims as the session authority', () => {
    const token = tokenFor({
      user_id: 'user_001',
      subject: 'root@example.com',
      role: 'admin',
      exp: 4_102_444_800,
    })

    expect(
      sessionFromAuthResponse({
        user_id: 'user_001',
        access_token: token,
        expire_at: 4_102_444_800,
      }),
    ).toMatchObject({
      accessToken: token,
      userId: 'user_001',
      subject: 'root@example.com',
      role: 'admin',
      expireAt: 4_102_444_800,
    })
  })
})

describe('session expiry helpers', () => {
  const baseSession: AuthSession = {
    accessToken: 'token',
    userId: 'user_001',
    subject: 'root@example.com',
    role: 'user',
    expireAt: 1_700_000_000,
  }

  test('treats sessions expiring at or before now as expired', () => {
    expect(isSessionExpired(baseSession, 1_700_000_000)).toBe(true)
    expect(isSessionExpired({ ...baseSession, expireAt: 1_699_999_999 }, 1_700_000_000)).toBe(true)
    expect(isSessionExpired({ ...baseSession, expireAt: 1_700_000_001 }, 1_700_000_000)).toBe(false)
  })

  test('rejects stored sessions without an access token or expiry', () => {
    expect(isStoredSessionUsable(null, 1_700_000_000)).toBe(false)
    expect(isStoredSessionUsable({ ...baseSession, accessToken: '' }, 1_700_000_000)).toBe(false)
    expect(isStoredSessionUsable({ ...baseSession, expireAt: 0 }, 1_700_000_000)).toBe(false)
    expect(isStoredSessionUsable(baseSession, 1_699_999_999)).toBe(true)
  })

  test('resolves the default workspace by role', () => {
    expect(redirectForRole('admin')).toBe('/admin')
    expect(redirectForRole('user')).toBe('/center')
    expect(redirectForRole('relay')).toBe('/center')
    expect(redirectForRole('agent')).toBe('/center')
  })

  test('keeps post-login redirects internal and role appropriate', () => {
    expect(safeRedirectTarget('/center/devices?status=online', 'user')).toBe('/center/devices?status=online')
    expect(safeRedirectTarget('/admin/users', 'user')).toBe('/center')
    expect(safeRedirectTarget('/center/devices', 'admin')).toBe('/admin')
    expect(safeRedirectTarget('https://example.com/phish', 'user')).toBe('/center')
    expect(safeRedirectTarget('//example.com/phish', 'admin')).toBe('/admin')
    expect(safeRedirectTarget(['/center/account'], 'user')).toBe('/center/account')
  })
})
