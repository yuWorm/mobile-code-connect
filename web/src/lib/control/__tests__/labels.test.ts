import { describe, expect, test } from 'bun:test'

import {
  formatCredentialStatus,
  formatAuditTargetType,
  formatDeviceAuthStatus,
  formatDeviceStatus,
  formatEnabledLabel,
  formatRelayHealth,
  formatRoleLabel,
  formatSessionStatus,
} from '../labels'

describe('control labels', () => {
  test('formats shared enabled and role labels for UI display', () => {
    expect(formatEnabledLabel(true)).toBe('已启用')
    expect(formatEnabledLabel(false)).toBe('已停用')
    expect(formatCredentialStatus(true)).toBe('已启用')
    expect(formatCredentialStatus(false)).toBe('已停用')
    expect(formatRoleLabel('admin')).toBe('管理员')
    expect(formatRoleLabel('user')).toBe('普通用户')
    expect(formatRoleLabel('custom')).toBe('custom')
    expect(formatAuditTargetType('user')).toBe('用户')
    expect(formatAuditTargetType('plan')).toBe('套餐')
    expect(formatAuditTargetType('relay')).toBe('Relay')
    expect(formatAuditTargetType('relay_credential')).toBe('Relay 凭据')
    expect(formatAuditTargetType('server_credential')).toBe('服务凭据')
    expect(formatAuditTargetType('oauth_identity')).toBe('OAuth 身份')
    expect(formatAuditTargetType('session')).toBe('会话')
    expect(formatAuditTargetType('custom')).toBe('custom')
  })

  test('formats device, relay, session, and device auth statuses', () => {
    expect(formatDeviceStatus('online')).toBe('在线')
    expect(formatDeviceStatus('offline')).toBe('离线')
    expect(formatDeviceStatus('maintenance')).toBe('maintenance')
    expect(formatRelayHealth(true)).toBe('健康')
    expect(formatRelayHealth(false)).toBe('异常')
    expect(formatSessionStatus('pending')).toBe('待认领')
    expect(formatSessionStatus('claimed')).toBe('已认领')
    expect(formatSessionStatus('bound')).toBe('已连接')
    expect(formatSessionStatus('closed')).toBe('已关闭')
    expect(formatSessionStatus('expired')).toBe('已过期')
    expect(formatDeviceAuthStatus('authorization_pending')).toBe('等待审批')
    expect(formatDeviceAuthStatus('slow_down')).toBe('轮询过快')
    expect(formatDeviceAuthStatus('approved')).toBe('已批准')
    expect(formatDeviceAuthStatus('denied')).toBe('已拒绝')
    expect(formatDeviceAuthStatus('expired')).toBe('已过期')
  })

  test('accepts a translator for localized UI labels', () => {
    const t = (key: string) => `translated:${key}`

    expect(formatEnabledLabel(true, t)).toBe('translated:label.enabled')
    expect(formatEnabledLabel(false, t)).toBe('translated:label.disabled')
    expect(formatCredentialStatus(true, t)).toBe('translated:label.enabled')
    expect(formatRoleLabel('admin', t)).toBe('translated:role.admin')
    expect(formatRoleLabel('user', t)).toBe('translated:role.user')
    expect(formatAuditTargetType('relay_credential', t)).toBe('translated:target.relayCredential')
    expect(formatDeviceStatus('online', t)).toBe('translated:status.online')
    expect(formatRelayHealth(false, t)).toBe('translated:status.unhealthy')
    expect(formatSessionStatus('bound', t)).toBe('translated:sessionStatus.bound')
    expect(formatDeviceAuthStatus('authorization_pending', t)).toBe('translated:deviceAuthStatus.authorizationPending')
    expect(formatDeviceStatus('maintenance', t)).toBe('maintenance')
  })
})
