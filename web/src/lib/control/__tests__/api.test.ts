import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

import { buildQueryPath, ControlApi, ControlApiError, controlApiErrorMessage } from '../api'

describe('buildQueryPath', () => {
  test('encodes list parameters and skips empty values', () => {
    const path = buildQueryPath('/users', {
      limit: 20,
      offset: 40,
      q: 'alice@example.com',
      role: '',
      enabled: true,
      sort: '-email',
    })

    expect(path).toBe('/users?limit=20&offset=40&sort=-email&q=alice%40example.com&enabled=true')
  })

  test('returns the original path when no parameters are present', () => {
    expect(buildQueryPath('/dashboard', {})).toBe('/dashboard')
  })
})

describe('controlApiErrorMessage', () => {
  test('maps login 401 responses to a clear credential error', () => {
    expect(
      controlApiErrorMessage(new ControlApiError(401, 'unauthorized'), {
        unauthorized: '邮箱或密码错误',
      }),
    ).toBe('邮箱或密码错误')
  })

  test('uses response bodies for non-empty API errors', () => {
    expect(controlApiErrorMessage(new ControlApiError(409, 'email is already registered'))).toBe(
      'email is already registered',
    )
  })
})

describe('ControlApi auth failure handling', () => {
  test('calls the auth failure hook for 401 responses', async () => {
    const originalFetch = globalThis.fetch
    const statuses: number[] = []
    globalThis.fetch = (async () => new Response('session expired', { status: 401 })) as typeof fetch

    try {
      const api = new ControlApi({
        baseUrl: 'https://control.test',
        token: 'token',
        onAuthFailure: (error) => {
          statuses.push(error.status)
        },
      })
      await expect(api.dashboard()).rejects.toBeInstanceOf(ControlApiError)
    } finally {
      globalThis.fetch = originalFetch
    }

    expect(statuses).toEqual([401])
  })

  test('does not call the auth failure hook for 403 responses', async () => {
    const originalFetch = globalThis.fetch
    const statuses: number[] = []
    globalThis.fetch = (async () => new Response('forbidden', { status: 403 })) as typeof fetch

    try {
      const api = new ControlApi({
        baseUrl: 'https://control.test',
        token: 'token',
        onAuthFailure: (error) => {
          statuses.push(error.status)
        },
      })
      await expect(api.dashboard()).rejects.toBeInstanceOf(ControlApiError)
    } finally {
      globalThis.fetch = originalFetch
    }

    expect(statuses).toEqual([])
  })
})

describe('relay legacy admin fields', () => {
  test('models relay admin address fields as optional compatibility data', () => {
    const source = readFileSync(new URL('../types.ts', import.meta.url), 'utf8')

    expect(source).toContain('admin_addr?: string')
    expect(source).toContain('admin_bound?: boolean')
    expect(source).not.toContain('admin_addr: string')
    expect(source).not.toContain('admin_bound: boolean')
  })
})

describe('server auth API paths', () => {
  test('posts device-code start and poll requests', async () => {
    const paths: string[] = []
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input, init) => {
      paths.push(`${init?.method} ${String(input).replace('https://control.test', '')}`)
      return new Response(
        JSON.stringify(
          paths.length === 1
            ? {
                device_code: 'device-code',
                user_code: 'ABCD-EFGH',
                verification_uri: '/server-auth/device',
                verification_uri_complete: '/server-auth/device?user_code=ABCD-EFGH',
                expires_in: 600,
                interval: 5,
              }
            : {
                status: 'authorization_pending',
                interval: 5,
              },
        ),
        { status: 200 },
      )
    }) as typeof fetch

    try {
      const api = new ControlApi({ baseUrl: 'https://control.test' })
      await api.startDeviceServerAuth({
        device_id: 'pc_001',
        device_name: 'Office PC',
        server_public_key: 'public-key',
      })
      await api.pollDeviceServerAuth({
        device_code: 'device-code',
        server_public_key: 'public-key',
      })
    } finally {
      globalThis.fetch = originalFetch
    }

    expect(paths).toEqual([
      'POST /server-auth/device/start',
      'POST /server-auth/device/poll',
    ])
  })

  test('approves and denies device-code sessions with encoded user codes', async () => {
    const paths: string[] = []
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input, init) => {
      paths.push(`${init?.method} ${String(input).replace('https://control.test', '')}`)
      return new Response(JSON.stringify({ user_code: 'ABCD EFGH', status: 'approved' }), {
        status: 200,
      })
    }) as typeof fetch

    try {
      const api = new ControlApi({ baseUrl: 'https://control.test' })
      await api.approveDeviceServerAuth('ABCD EFGH')
      await api.denyDeviceServerAuth('ABCD EFGH')
    } finally {
      globalThis.fetch = originalFetch
    }

    expect(paths).toEqual([
      'GET /server-auth/device?user_code=ABCD+EFGH',
      'GET /server-auth/device?user_code=ABCD+EFGH&decision=deny',
    ])
  })
})

describe('github oauth API paths', () => {
  test('calls the backend callback endpoint with code and state', async () => {
    const paths: string[] = []
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input, init) => {
      paths.push(`${init?.method} ${String(input).replace('https://control.test', '')}`)
      return new Response(
        JSON.stringify({
          user_id: 'user_001',
          access_token: 'header.payload.signature',
          expire_at: 1_760_000_000,
        }),
        { status: 200 },
      )
    }) as typeof fetch

    try {
      const api = new ControlApi({ baseUrl: 'https://control.test' })
      await api.githubOAuthCallback({ code: 'code 1', state: 'state/1' })
    } finally {
      globalThis.fetch = originalFetch
    }

    expect(paths).toEqual([
      'GET /auth/oauth/github/callback?code=code+1&state=state%2F1',
    ])
  })
})

describe('relay bootstrap API paths', () => {
  test('creates and exchanges relay bootstrap tokens', async () => {
    const requests: Array<{ path: string, body: unknown }> = []
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input, init) => {
      requests.push({
        path: `${init?.method} ${String(input).replace('https://control.test', '')}`,
        body: init?.body ? JSON.parse(String(init.body)) : null,
      })
      return new Response(
        JSON.stringify(
          requests.length === 1
            ? {
                bootstrap_id: 'rb_001',
                relay_id: 'relay_bootstrap',
                control_url: 'https://control.test',
                expires_epoch_sec: 1_760_000_000,
                install_command: 'install command',
                no_service_install_command: 'no-service install command',
                bootstrap_token: 'shown-once',
              }
            : {
                control_url: 'https://control.test',
                control_token: 'relay-control-token',
                relay_id: 'relay_bootstrap',
                token_secret: 'relay-token-secret',
                relay_addr: 'relay.example.com:4433',
                admin_addr: '',
                capacity_streams: 128,
                heartbeat_interval_sec: 30,
              },
        ),
        { status: 200 },
      )
    }) as typeof fetch

    try {
      const api = new ControlApi({ baseUrl: 'https://control.test' })
      await api.createRelayBootstrap({
        relay_id: 'relay_bootstrap',
        control_url: 'https://control.test',
        relay_addr: 'relay.example.com:4433',
        admin_addr: '',
        capacity_streams: 128,
        heartbeat_interval_sec: 30,
        ttl_sec: 900,
      })
      await api.exchangeRelayBootstrap('rb_001', { bootstrap_token: 'shown-once' })
    } finally {
      globalThis.fetch = originalFetch
    }

    expect(requests).toEqual([
      {
        path: 'POST /relay-bootstraps',
        body: {
          relay_id: 'relay_bootstrap',
          control_url: 'https://control.test',
          relay_addr: 'relay.example.com:4433',
          admin_addr: '',
          capacity_streams: 128,
          heartbeat_interval_sec: 30,
          ttl_sec: 900,
        },
      },
      {
        path: 'POST /relay-bootstraps/rb_001/exchange',
        body: { bootstrap_token: 'shown-once' },
      },
    ])
  })
})

describe('relay live ops API paths', () => {
  test('lists relay sessions and queues disconnect commands through Control', async () => {
    const originalFetch = globalThis.fetch
    const requests: Array<{ path: string; body: unknown }> = []
    globalThis.fetch = (async (input, init) => {
      const url = new URL(String(input))
      requests.push({
        path: `${init?.method ?? 'GET'} ${url.pathname}${url.search}`,
        body: init?.body ? JSON.parse(String(init.body)) : null,
      })
      return new Response(
        JSON.stringify(
          url.pathname.endsWith('/disconnect')
            ? {
                command_id: 'rc_001',
                relay_id: 'relay_live_ops',
                kind: 'disconnect_session',
                session_id: 'sess_live_ops_001',
                status: 'pending',
                requested_epoch_sec: 1781097600,
                updated_epoch_sec: 1781097600,
                message: '',
              }
            : {
                items: [
                  {
                    session_id: 'sess_live_ops_001',
                    state: 'ready',
                    mobile_bound: true,
                    agent_bound: true,
                    limits: {
                      max_bps: 8192,
                      max_streams: 16,
                      max_duration_sec: 3600,
                      traffic_quota_bytes: 1048576,
                    },
                    stats: {
                      session_id: 'sess_live_ops_001',
                      uplink_bytes: 1024,
                      downlink_bytes: 2048,
                      total_bytes: 3072,
                      duration_sec: 30,
                      active_streams: 2,
                    },
                    last_seen_epoch_sec: 1781097600,
                  },
                ],
                total: 1,
                limit: 10,
                offset: 0,
              },
        ),
        { status: 200 },
      )
    }) as typeof fetch

    try {
      const api = new ControlApi({ baseUrl: 'https://control.test' })
      await api.relaySessions('relay_live_ops', { limit: 10, status: 'ready' })
      await api.disconnectRelaySession('relay_live_ops', 'sess_live_ops_001')
    } finally {
      globalThis.fetch = originalFetch
    }

    expect(requests).toEqual([
      {
        path: 'GET /relays/relay_live_ops/sessions?limit=10&status=ready',
        body: null,
      },
      {
        path: 'POST /relays/relay_live_ops/sessions/sess_live_ops_001/disconnect',
        body: null,
      },
    ])
  })
})
