import type {
  AdminListQuery,
  AdminSessionSummary,
  AssignUserPlanRequest,
  AuditLogEntry,
  AuthResponse,
  BrowserServerAuthApprovalResponse,
  CreateRelayCredentialRequest,
  CreateRelayBootstrapRequest,
  CreateSessionRequest,
  CreateSessionResponse,
  CreateUserRequest,
  DashboardSummary,
  Device,
  DeviceAccessGrant,
  DeviceServerAuthApprovalResponse,
  DeviceServerAuthPollResponse,
  DeviceServerAuthStartResponse,
  GrantDeviceAccessRequest,
  GitHubOAuthCallbackRequest,
  LoginRequest,
  OAuthIdentity,
  Page,
  Plan,
  RegisterControllerDeviceRequest,
  RelayBootstrapExchangeRequest,
  RelayBootstrapExchangeResponse,
  RelayBootstrapResponse,
  RelayCommand,
  RegisterRelayRequest,
  RegisterUserRequest,
  RelayCredential,
  RelayNode,
  RelaySessionSnapshot,
  ReportRelayHealthRequest,
  ServerCredentialResponse,
  ServerCredentialSummary,
  ServerAuthSessionDetail,
  PollServerAuthRequest,
  StartServerAuthRequest,
  UpdatePasswordRequest,
  UpdatePlanCatalogRequest,
  UpdateRelayCredentialStatusRequest,
  UpdateRelayRequest,
  UpdateServerCredentialStatusRequest,
  UpdateUserPlanRequest,
  UpdateUserRoleRequest,
  UpdateUserStatusRequest,
  UserDetail,
  UserSummary,
  UserUsagePeriod,
  UserUsageSummary,
  ControllerDevice,
  Service,
} from './types'

const queryOrder: (keyof AdminListQuery)[] = [
  'limit',
  'offset',
  'sort',
  'q',
  'role',
  'enabled',
  'status',
  'user_id',
  'device_id',
  'healthy',
  'action',
  'target_type',
]

export class ControlApiError extends Error {
  public readonly status: number
  public readonly body: string

  constructor(status: number, body: string) {
    super(body || `Control API request failed with ${status}`)
    this.status = status
    this.body = body
  }
}

export interface ControlApiErrorMessageOptions {
  unauthorized?: string
  forbidden?: string
  fallback?: string
}

export function controlApiErrorMessage(
  error: unknown,
  options: ControlApiErrorMessageOptions = {},
) {
  if (error instanceof ControlApiError) {
    if (error.status === 401 && options.unauthorized) {
      return options.unauthorized
    }
    if (error.status === 403 && options.forbidden) {
      return options.forbidden
    }
    return error.body || options.fallback || error.message
  }
  if (error instanceof Error) {
    return error.message
  }
  return options.fallback ?? '请求失败'
}

export function buildQueryPath(path: string, query: AdminListQuery = {}) {
  const params = new URLSearchParams()
  for (const key of queryOrder) {
    const value = query[key]
    if (value === undefined || value === null) {
      continue
    }
    if (typeof value === 'string' && value.trim() === '') {
      continue
    }
    params.set(key, String(value))
  }
  const queryString = params.toString()
  return queryString ? `${path}?${queryString}` : path
}

export interface ControlApiOptions {
  baseUrl?: string
  token?: string | null
  onAuthFailure?: ControlApiAuthFailureHandler | null
}

export type ControlApiAuthFailureHandler = (error: ControlApiError) => void

export class ControlApi {
  private baseUrl: string
  private token: string | null
  private onAuthFailure: ControlApiAuthFailureHandler | null

  constructor(options: ControlApiOptions = {}) {
    this.baseUrl = (options.baseUrl ?? '').replace(/\/$/, '')
    this.token = options.token ?? null
    this.onAuthFailure = options.onAuthFailure ?? null
  }

  setBaseUrl(baseUrl: string) {
    this.baseUrl = baseUrl.replace(/\/$/, '')
  }

  setToken(token: string | null) {
    this.token = token
  }

  setAuthFailureHandler(handler: ControlApiAuthFailureHandler | null) {
    this.onAuthFailure = handler
  }

  register(request: RegisterUserRequest) {
    return this.post<AuthResponse>('/auth/register', request)
  }

  login(request: LoginRequest) {
    return this.post<AuthResponse>('/auth/login', request)
  }

  githubOAuthCallback(request: GitHubOAuthCallbackRequest) {
    const params = new URLSearchParams({ code: request.code, state: request.state })
    return this.get<AuthResponse>(`/auth/oauth/github/callback?${params.toString()}`)
  }

  updatePassword(request: UpdatePasswordRequest) {
    return this.postEmpty('/auth/password', request)
  }

  dashboard() {
    return this.get<DashboardSummary>('/dashboard')
  }

  users(query?: AdminListQuery) {
    return this.get<Page<UserSummary>>(buildQueryPath('/users', query))
  }

  createUser(request: CreateUserRequest) {
    return this.post<UserSummary>('/users', request)
  }

  user(userId: string) {
    return this.get<UserDetail>(`/users/${encodeURIComponent(userId)}`)
  }

  updateUserStatus(userId: string, request: UpdateUserStatusRequest) {
    return this.post<UserSummary>(`/users/${encodeURIComponent(userId)}/status`, request)
  }

  updateUserRole(userId: string, request: UpdateUserRoleRequest) {
    return this.post<UserSummary>(`/users/${encodeURIComponent(userId)}/role`, request)
  }

  userUsage(query?: AdminListQuery) {
    return this.get<Page<UserUsageSummary>>(buildQueryPath('/usage/users', query))
  }

  resetUserUsage(userId: string) {
    return this.postNoBody<UserUsagePeriod>(`/usage/users/${encodeURIComponent(userId)}/reset`)
  }

  auditLogs(query?: AdminListQuery) {
    return this.get<Page<AuditLogEntry>>(buildQueryPath('/audit-logs', query))
  }

  controllers(query?: AdminListQuery) {
    return this.get<Page<ControllerDevice>>(buildQueryPath('/controllers', query))
  }

  registerController(request: RegisterControllerDeviceRequest) {
    return this.post<ControllerDevice>('/controllers/register', request)
  }

  removeController(clientId: string) {
    return this.deleteEmpty(`/controllers/${encodeURIComponent(clientId)}`)
  }

  devices(query?: AdminListQuery) {
    return this.get<Page<Device>>(buildQueryPath('/devices', query))
  }

  mobileDevices() {
    return this.get<Device[]>('/mobile/devices')
  }

  device(deviceId: string) {
    return this.get<Device>(`/devices/${encodeURIComponent(deviceId)}`)
  }

  removeDevice(deviceId: string) {
    return this.deleteEmpty(`/devices/${encodeURIComponent(deviceId)}`)
  }

  deviceServices(deviceId: string) {
    return this.get<Service[]>(`/mobile/devices/${encodeURIComponent(deviceId)}/services`)
  }

  deviceAccess(deviceId: string, query?: AdminListQuery) {
    return this.get<Page<DeviceAccessGrant>>(
      buildQueryPath(`/devices/${encodeURIComponent(deviceId)}/access`, query),
    )
  }

  grantDeviceAccess(deviceId: string, request: GrantDeviceAccessRequest) {
    return this.post<DeviceAccessGrant>(`/devices/${encodeURIComponent(deviceId)}/access`, request)
  }

  revokeDeviceAccess(deviceId: string, userId: string) {
    return this.deleteEmpty(
      `/devices/${encodeURIComponent(deviceId)}/access/${encodeURIComponent(userId)}`,
    )
  }

  currentPlan() {
    return this.get<Plan>('/plans/current')
  }

  planCatalog(query?: AdminListQuery) {
    return this.get<Page<Plan>>(buildQueryPath('/plans/catalog', query))
  }

  updatePlanCatalog(request: UpdatePlanCatalogRequest) {
    return this.post<Plan>('/plans/catalog', request)
  }

  userPlan(userId: string) {
    return this.get<Plan>(`/plans/users/${encodeURIComponent(userId)}`)
  }

  assignUserPlan(userId: string, request: AssignUserPlanRequest) {
    return this.post<Plan>(`/plans/users/${encodeURIComponent(userId)}/assign`, request)
  }

  updateUserPlan(userId: string, request: UpdateUserPlanRequest) {
    return this.post<Plan>(`/plans/users/${encodeURIComponent(userId)}`, request)
  }

  relays(query?: AdminListQuery) {
    return this.get<Page<RelayNode>>(buildQueryPath('/relays', query))
  }

  relaySessions(relayId: string, query?: AdminListQuery) {
    return this.get<Page<RelaySessionSnapshot>>(
      buildQueryPath(`/relays/${encodeURIComponent(relayId)}/sessions`, query),
    )
  }

  disconnectRelaySession(relayId: string, sessionId: string) {
    return this.postNoBody<RelayCommand>(
      `/relays/${encodeURIComponent(relayId)}/sessions/${encodeURIComponent(sessionId)}/disconnect`,
    )
  }

  registerRelay(request: RegisterRelayRequest) {
    return this.post<RelayNode>('/relays/register', request)
  }

  updateRelay(relayId: string, request: UpdateRelayRequest) {
    return this.post<RelayNode>(`/relays/${encodeURIComponent(relayId)}`, request)
  }

  reportRelayHealth(relayId: string, request: ReportRelayHealthRequest) {
    return this.post<RelayNode>(`/relays/${encodeURIComponent(relayId)}/health`, request)
  }

  removeRelay(relayId: string) {
    return this.deleteEmpty(`/relays/${encodeURIComponent(relayId)}`)
  }

  relayCredentials(query?: AdminListQuery) {
    return this.get<Page<RelayCredential>>(buildQueryPath('/relay-credentials', query))
  }

  createRelayCredential(request: CreateRelayCredentialRequest) {
    return this.post<RelayCredential>('/relay-credentials', request)
  }

  updateRelayCredentialStatus(relayId: string, request: UpdateRelayCredentialStatusRequest) {
    return this.post<RelayCredential>(
      `/relay-credentials/${encodeURIComponent(relayId)}/status`,
      request,
    )
  }

  rotateRelayCredential(relayId: string) {
    return this.postNoBody<RelayCredential>(
      `/relay-credentials/${encodeURIComponent(relayId)}/rotate`,
    )
  }

  createRelayBootstrap(request: CreateRelayBootstrapRequest) {
    return this.post<RelayBootstrapResponse>('/relay-bootstraps', request)
  }

  exchangeRelayBootstrap(bootstrapId: string, request: RelayBootstrapExchangeRequest) {
    return this.post<RelayBootstrapExchangeResponse>(
      `/relay-bootstraps/${encodeURIComponent(bootstrapId)}/exchange`,
      request,
    )
  }

  serverCredentials(query?: AdminListQuery) {
    return this.get<Page<ServerCredentialSummary>>(buildQueryPath('/server-credentials', query))
  }

  updateServerCredentialStatus(
    credentialId: string,
    request: UpdateServerCredentialStatusRequest,
  ) {
    return this.post<ServerCredentialSummary>(
      `/server-credentials/${encodeURIComponent(credentialId)}/status`,
      request,
    )
  }

  rotateServerCredential(credentialId: string) {
    return this.postNoBody<ServerCredentialResponse>(
      `/server-credentials/${encodeURIComponent(credentialId)}/rotate`,
    )
  }

  oauthIdentities(query?: AdminListQuery) {
    return this.get<Page<OAuthIdentity>>(buildQueryPath('/oauth/identities', query))
  }

  unlinkOAuthIdentity(provider: 'github', providerUserId: string) {
    return this.deleteEmpty(
      `/oauth/identities/${provider}/${encodeURIComponent(providerUserId)}`,
    )
  }

  browserServerAuthSessionDetail(sessionId: string) {
    const params = new URLSearchParams({ session_id: sessionId })
    return this.get<ServerAuthSessionDetail>(`/server-auth/browser/session?${params.toString()}`)
  }

  approveBrowserServerAuth(sessionId: string) {
    const params = new URLSearchParams({ session_id: sessionId })
    return this.get<BrowserServerAuthApprovalResponse>(`/server-auth/browser/approve?${params.toString()}`)
  }

  deviceServerAuthSessionDetail(userCode: string) {
    const params = new URLSearchParams({ user_code: userCode })
    return this.get<ServerAuthSessionDetail>(`/server-auth/device/session?${params.toString()}`)
  }

  startDeviceServerAuth(request: StartServerAuthRequest) {
    return this.post<DeviceServerAuthStartResponse>('/server-auth/device/start', request)
  }

  approveDeviceServerAuth(userCode: string) {
    const params = new URLSearchParams({ user_code: userCode })
    return this.get<DeviceServerAuthApprovalResponse>(`/server-auth/device?${params.toString()}`)
  }

  denyDeviceServerAuth(userCode: string) {
    const params = new URLSearchParams({ user_code: userCode, decision: 'deny' })
    return this.get<DeviceServerAuthApprovalResponse>(`/server-auth/device?${params.toString()}`)
  }

  pollDeviceServerAuth(request: PollServerAuthRequest) {
    return this.post<DeviceServerAuthPollResponse>('/server-auth/device/poll', request)
  }

  sessions(query?: AdminListQuery) {
    return this.get<Page<AdminSessionSummary>>(buildQueryPath('/sessions', query))
  }

  createSession(request: CreateSessionRequest) {
    return this.post<CreateSessionResponse>('/sessions', request)
  }

  closeSession(sessionId: string) {
    return this.postNoBody<unknown>(`/sessions/${encodeURIComponent(sessionId)}/close`)
  }

  private async get<T>(path: string) {
    return this.request<T>('GET', path)
  }

  private async post<T>(path: string, body: unknown) {
    return this.request<T>('POST', path, body)
  }

  private async postNoBody<T>(path: string) {
    return this.request<T>('POST', path)
  }

  private async postEmpty(path: string, body: unknown) {
    await this.request<void>('POST', path, body, false)
  }

  private async deleteEmpty(path: string) {
    await this.request<void>('DELETE', path, undefined, false)
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
    decodeJson = true,
  ): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers: {
        ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
        ...(this.token ? { Authorization: `Bearer ${this.token}` } : {}),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    })
    if (!response.ok) {
      const error = new ControlApiError(response.status, await response.text())
      if (response.status === 401) {
        this.onAuthFailure?.(error)
      }
      throw error
    }
    if (!decodeJson || response.status === 204) {
      return undefined as T
    }
    return (await response.json()) as T
  }
}
