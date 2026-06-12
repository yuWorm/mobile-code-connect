type LabelTranslator = (key: string) => string

function translated(t: LabelTranslator | undefined, key: string, fallback: string) {
  return t ? t(key) : fallback
}

export function formatEnabledLabel(enabled: boolean, t?: LabelTranslator) {
  return enabled
    ? translated(t, 'label.enabled', '已启用')
    : translated(t, 'label.disabled', '已停用')
}

export function formatCredentialStatus(enabled: boolean, t?: LabelTranslator) {
  return formatEnabledLabel(enabled, t)
}

export function formatRoleLabel(role?: string | null, t?: LabelTranslator) {
  if (role === 'admin') return translated(t, 'role.admin', '管理员')
  if (role === 'user') return translated(t, 'role.user', '普通用户')
  return role || '-'
}

export function formatAuditTargetType(targetType: string, t?: LabelTranslator) {
  const labels: Record<string, { key: string; fallback: string }> = {
    user: { key: 'target.user', fallback: '用户' },
    plan: { key: 'target.plan', fallback: '套餐' },
    relay: { key: 'target.relay', fallback: 'Relay' },
    relay_credential: { key: 'target.relayCredential', fallback: 'Relay 凭据' },
    server_credential: { key: 'target.serverCredential', fallback: '服务凭据' },
    oauth_identity: { key: 'target.oauthIdentity', fallback: 'OAuth 身份' },
    session: { key: 'target.session', fallback: '会话' },
  }
  const label = labels[targetType]
  return label ? translated(t, label.key, label.fallback) : targetType
}

export function formatDeviceStatus(status: string, t?: LabelTranslator) {
  if (status === 'online') return translated(t, 'status.online', '在线')
  if (status === 'offline') return translated(t, 'status.offline', '离线')
  return status
}

export function formatRelayHealth(healthy: boolean, t?: LabelTranslator) {
  return healthy
    ? translated(t, 'status.healthy', '健康')
    : translated(t, 'status.unhealthy', '异常')
}

export function formatSessionStatus(status: string, t?: LabelTranslator) {
  const labels: Record<string, { key: string; fallback: string }> = {
    pending: { key: 'sessionStatus.pending', fallback: '待认领' },
    claimed: { key: 'sessionStatus.claimed', fallback: '已认领' },
    bound: { key: 'sessionStatus.bound', fallback: '已连接' },
    closed: { key: 'sessionStatus.closed', fallback: '已关闭' },
    expired: { key: 'sessionStatus.expired', fallback: '已过期' },
  }
  const label = labels[status]
  return label ? translated(t, label.key, label.fallback) : status
}

export function formatDeviceAuthStatus(status: string, t?: LabelTranslator) {
  const labels: Record<string, { key: string; fallback: string }> = {
    authorization_pending: { key: 'deviceAuthStatus.authorizationPending', fallback: '等待审批' },
    slow_down: { key: 'deviceAuthStatus.slowDown', fallback: '轮询过快' },
    approved: { key: 'deviceAuthStatus.approved', fallback: '已批准' },
    denied: { key: 'deviceAuthStatus.denied', fallback: '已拒绝' },
    expired: { key: 'deviceAuthStatus.expired', fallback: '已过期' },
  }
  const label = labels[status]
  return label ? translated(t, label.key, label.fallback) : status
}
