import type { AdminSessionSummary, CreateSessionResponse } from './types'

export interface CreatedSessionClipboardContext {
  clientId?: string | null
  deviceId?: string | null
  deviceName?: string | null
  serviceId?: string | null
  serviceName?: string | null
}

export function sessionExpiryIso(expireAt?: number | null) {
  if (!expireAt) {
    return '-'
  }
  return new Date(expireAt * 1000).toISOString()
}

export function buildCreatedSessionClipboardText(
  session: CreateSessionResponse,
  context: CreatedSessionClipboardContext = {},
) {
  const device = context.deviceName
    ? `${context.deviceName} (${context.deviceId ?? '-'})`
    : (context.deviceId ?? '-')
  const service = context.serviceName
    ? `${context.serviceName} (${context.serviceId ?? '-'})`
    : (context.serviceId ?? '-')

  return [
    `Session ID: ${session.session_id}`,
    `Client ID: ${context.clientId ?? '-'}`,
    `Device: ${device}`,
    `Service: ${service}`,
    `Relay Address: ${session.relay_addr}`,
    `Punch Address: ${session.punch_addr}`,
    `Expires At: ${sessionExpiryIso(session.expire_at)}`,
    `Access Token: ${session.access_token}`,
    `Relay Token: ${session.relay_token}`,
  ].join('\n')
}

export function canCloseAdminSession(session: Pick<AdminSessionSummary, 'status'>) {
  return session.status !== 'closed' && session.status !== 'expired'
}
