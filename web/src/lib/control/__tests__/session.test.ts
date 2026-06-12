import { describe, expect, test } from 'bun:test'

import {
  buildCreatedSessionClipboardText,
  canCloseAdminSession,
  sessionExpiryIso,
} from '../session'
import type { AdminSessionSummary, CreateSessionResponse } from '../types'

const createdSession: CreateSessionResponse = {
  session_id: 'sess_001',
  access_token: 'access-token',
  relay_token: 'relay-token',
  relay_addr: 'relay.example.com:4433',
  punch_addr: 'punch.example.com:4433',
  agent_p2p_cert_der: [1, 2, 3],
  expire_at: 1_750_000_000,
}

describe('session helpers', () => {
  test('builds a copyable session credential bundle with selected targets', () => {
    expect(
      buildCreatedSessionClipboardText(createdSession, {
        clientId: 'phone_1',
        deviceId: 'dev_1',
        deviceName: 'Office Mac',
        serviceId: 'ssh',
        serviceName: 'SSH',
      }),
    ).toBe(
      [
        'Session ID: sess_001',
        'Client ID: phone_1',
        'Device: Office Mac (dev_1)',
        'Service: SSH (ssh)',
        'Relay Address: relay.example.com:4433',
        'Punch Address: punch.example.com:4433',
        'Expires At: 2025-06-15T15:06:40.000Z',
        'Access Token: access-token',
        'Relay Token: relay-token',
      ].join('\n'),
    )
  })

  test('formats missing session expiry as a placeholder', () => {
    expect(sessionExpiryIso(null)).toBe('-')
  })

  test('only allows active admin sessions to be closed', () => {
    const session = { status: 'bound' } as AdminSessionSummary
    expect(canCloseAdminSession(session)).toBe(true)

    expect(canCloseAdminSession({ ...session, status: 'closed' })).toBe(false)
    expect(canCloseAdminSession({ ...session, status: 'expired' })).toBe(false)
  })
})
