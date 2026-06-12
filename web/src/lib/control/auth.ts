import type { AuthResponse, AuthSession, ControlClaims } from './types'

const storageKey = 'quic-control-session'

export function decodeControlClaims(token: string): ControlClaims {
  const [payload] = token.split('.')
  if (!payload) {
    throw new Error('Control token is missing a payload')
  }
  return JSON.parse(decodeBase64Url(payload)) as ControlClaims
}

export function sessionFromAuthResponse(response: AuthResponse): AuthSession {
  const claims = decodeControlClaims(response.access_token)
  return {
    accessToken: response.access_token,
    userId: claims.user_id || response.user_id,
    subject: claims.subject,
    role: claims.role,
    expireAt: claims.exp || response.expire_at,
  }
}

export function currentEpochSeconds() {
  return Math.floor(Date.now() / 1000)
}

export function isSessionExpired(session: AuthSession, nowSeconds = currentEpochSeconds()) {
  return !session.expireAt || session.expireAt <= nowSeconds
}

export function isStoredSessionUsable(session: AuthSession | null, nowSeconds = currentEpochSeconds()) {
  if (!session?.accessToken || !session.expireAt) {
    return false
  }
  return !isSessionExpired(session, nowSeconds)
}

export function redirectForRole(role?: AuthSession['role'] | null) {
  return role === 'admin' ? '/admin' : '/center'
}

export function safeRedirectTarget(redirect: unknown, role?: AuthSession['role'] | null) {
  const fallback = redirectForRole(role)
  const target = Array.isArray(redirect) ? redirect.find((value) => typeof value === 'string') : redirect

  if (typeof target !== 'string' || !target.startsWith('/') || target.startsWith('//')) {
    return fallback
  }
  if (role !== 'admin' && isRouteUnder(target, '/admin')) {
    return fallback
  }
  if (role === 'admin' && isRouteUnder(target, '/center')) {
    return fallback
  }
  return target
}

export function readStoredSession(): AuthSession | null {
  const raw = localStorage.getItem(storageKey)
  if (!raw) {
    return null
  }
  try {
    return JSON.parse(raw) as AuthSession
  } catch {
    localStorage.removeItem(storageKey)
    return null
  }
}

export function writeStoredSession(session: AuthSession) {
  localStorage.setItem(storageKey, JSON.stringify(session))
}

export function clearStoredSession() {
  localStorage.removeItem(storageKey)
}

function isRouteUnder(target: string, basePath: string) {
  return target === basePath || target.startsWith(`${basePath}/`) || target.startsWith(`${basePath}?`) || target.startsWith(`${basePath}#`)
}

function decodeBase64Url(value: string) {
  const normalized = value.replaceAll('-', '+').replaceAll('_', '/')
  const padding = '='.repeat((4 - (normalized.length % 4)) % 4)
  return decodeURIComponent(
    Array.from(atob(normalized + padding))
      .map((char) => `%${char.charCodeAt(0).toString(16).padStart(2, '0')}`)
      .join(''),
  )
}
