export type ControlRole = 'user' | 'admin' | 'relay' | 'agent'

export interface Page<T> {
  items: T[]
  total: number
  limit: number
  offset: number
}

export interface AdminListQuery {
  limit?: number
  offset?: number
  sort?: string
  q?: string
  role?: string
  enabled?: boolean
  status?: string
  user_id?: string
  device_id?: string
  healthy?: boolean
  action?: string
  target_type?: string
}

export interface AuthResponse {
  user_id: string
  access_token: string
  expire_at: number
}

export interface ControlClaims {
  user_id: string
  subject: string
  role: ControlRole
  exp: number
  relay_token_version?: number | null
  credential_id?: string | null
  server_credential_version?: number | null
}

export interface AuthSession {
  accessToken: string
  userId: string
  subject: string
  role: ControlRole
  expireAt: number
}

export interface LoginRequest {
  email: string
  password: string
}

export interface GitHubOAuthCallbackRequest {
  code: string
  state: string
}

export interface RegisterUserRequest {
  email: string
  password: string
  display_name: string
}

export interface UpdatePasswordRequest {
  current_password?: string | null
  new_password: string
}

export interface DashboardSummary {
  users: DashboardUserStats
  devices: DashboardDeviceStats
  controllers: DashboardControllerStats
  sessions: DashboardSessionStats
  relays: DashboardRelayStats
  usage: DashboardUsageStats
  recent_audit_logs: AuditLogEntry[]
}

export interface DashboardUserStats {
  total: number
  enabled: number
  admins: number
}

export interface DashboardDeviceStats {
  total: number
  online: number
}

export interface DashboardControllerStats {
  total: number
}

export interface DashboardSessionStats {
  total: number
  pending: number
  claimed: number
  bound: number
  closed: number
  expired: number
}

export interface DashboardRelayStats {
  total: number
  healthy: number
  unhealthy: number
}

export interface DashboardUsageStats {
  actual_uplink_bytes: number
  actual_downlink_bytes: number
  actual_total_bytes: number
}

export interface AuditLogEntry {
  audit_id: string
  actor_user_id: string
  actor_subject: string
  actor_role: ControlRole
  action: string
  target_type: string
  target_id: string
  message: string
  created_epoch_sec: number
}

export interface UserSummary {
  user_id: string
  email: string
  display_name: string
  role: ControlRole
  enabled: boolean
  plan_id: string
  controller_count: number
  device_count: number
}

export interface UserDetail {
  user: UserSummary
  plan: Plan
  controllers: ControllerDevice[]
  devices: Device[]
}

export interface UserUsageSummary {
  user_id: string
  email: string
  plan_id: string
  current_period_started_epoch_sec: number
  max_controller_devices: number
  controller_count: number
  device_count: number
  session_count: number
  pending_sessions: number
  claimed_sessions: number
  bound_sessions: number
  closed_sessions: number
  expired_sessions: number
  current_session_quota_bytes: number
  relay_quota_granted_bytes: number
  actual_uplink_bytes: number
  actual_downlink_bytes: number
  actual_total_bytes: number
}

export interface UserUsagePeriod {
  user_id: string
  current_period_started_epoch_sec: number
}

export interface CreateUserRequest {
  email: string
  password: string
  display_name: string
  role: ControlRole
  enabled: boolean
}

export interface UpdateUserStatusRequest {
  enabled: boolean
}

export interface UpdateUserRoleRequest {
  role: ControlRole
}

export interface ControllerDevice {
  user_id: string
  client_id: string
  name: string
}

export interface RegisterControllerDeviceRequest {
  client_id: string
  name: string
}

export interface Device {
  device_id: string
  user_id: string
  name: string
  status: 'online' | 'offline'
  agent_version: string
}

export interface DeviceAccessGrant {
  device_id: string
  user_id: string
}

export interface GrantDeviceAccessRequest {
  user_id: string
}

export interface Service {
  service_id: string
  device_id: string
  name: string
  protocol: 'tcp'
  target_host: string
  target_port: number
}

export interface Plan {
  plan_id: string
  name: string
  max_controller_devices: number
  relay_limits: RelayLimits
}

export interface RelayLimits {
  max_bps: number
  max_streams: number
  max_duration_sec: number
  traffic_quota_bytes: number
}

export interface UpdatePlanCatalogRequest {
  plan: Plan
}

export interface UpdateUserPlanRequest {
  plan: Plan
}

export interface AssignUserPlanRequest {
  plan_id: string
}

export interface RelayCredential {
  relay_id: string
  enabled: boolean
  token_version: number
}

export interface CreateRelayCredentialRequest {
  relay_id: string
  enabled: boolean
}

export interface UpdateRelayCredentialStatusRequest {
  enabled: boolean
}

export interface CreateRelayBootstrapRequest {
  relay_id: string
  control_url: string
  relay_addr: string
  admin_addr?: string
  capacity_streams: number
  heartbeat_interval_sec: number
  ttl_sec: number
}

export interface RelayBootstrapResponse {
  bootstrap_id: string
  relay_id: string
  control_url: string
  expires_epoch_sec: number
  install_command: string
  no_service_install_command: string
  bootstrap_token: string
}

export interface RelayBootstrapExchangeRequest {
  bootstrap_token: string
}

export interface RelayBootstrapExchangeResponse {
  control_url: string
  control_token: string
  relay_id: string
  token_secret: string
  relay_addr: string
  admin_addr?: string
  capacity_streams: number
  heartbeat_interval_sec: number
}

export type RelayHealthStatus = 'healthy' | 'degraded' | 'unhealthy'

export interface RelayHealthReport {
  status: RelayHealthStatus
  reason: string
  relay_version: string
  uptime_sec: number
  active_sessions: number
  active_streams: number
  total_uplink_bytes: number
  total_downlink_bytes: number
  total_bytes: number
  data_plane_bound: boolean
  admin_bound?: boolean
}

export interface TrafficStats {
  session_id?: string | null
  uplink_bytes: number
  downlink_bytes: number
  total_bytes: number
  duration_sec: number
  active_streams: number
}

export interface RelaySessionSnapshot {
  session_id: string
  state: string
  mobile_bound: boolean
  agent_bound: boolean
  limits: RelayLimits
  stats: TrafficStats
  last_seen_epoch_sec: number
}

export type RelayCommandKind = 'disconnect_session'
export type RelayCommandStatus = 'pending' | 'succeeded' | 'failed'

export interface RelayCommand {
  command_id: string
  relay_id: string
  kind: RelayCommandKind
  session_id?: string | null
  status: RelayCommandStatus
  requested_epoch_sec: number
  updated_epoch_sec: number
  message: string
}

export interface ReportRelayHealthRequest {
  relay_addr: string
  admin_addr?: string
  capacity_streams: number
  health: RelayHealthReport
  sessions?: RelaySessionSnapshot[]
}

export interface RelayNode {
  relay_id: string
  relay_addr: string
  admin_addr?: string
  capacity_streams: number
  healthy: boolean
  last_seen_epoch_sec: number
  health_status: RelayHealthStatus
  health_reason: string
  relay_version: string
  uptime_sec: number
  active_sessions: number
  active_streams: number
  total_uplink_bytes: number
  total_downlink_bytes: number
  total_bytes: number
  data_plane_bound: boolean
  admin_bound?: boolean
  last_health_report_epoch_sec: number
}

export interface RegisterRelayRequest {
  relay_id: string
  relay_addr: string
  admin_addr?: string
  capacity_streams: number
}

export interface UpdateRelayRequest extends RegisterRelayRequest {
  healthy: boolean
}

export interface ServerCredentialSummary {
  credential_id: string
  user_id: string
  device_id: string
  device_name: string
  enabled: boolean
  token_version: number
  created_epoch_sec: number
  last_used_epoch_sec?: number | null
}

export interface ServerCredentialResponse {
  credential_id: string
  device_id: string
  server_token: string
  token_type: string
}

export interface UpdateServerCredentialStatusRequest {
  enabled: boolean
}

export interface StartServerAuthRequest {
  device_id: string
  device_name: string
  server_public_key: string
}

export type ServerAuthStatus =
  | 'pending'
  | 'approved'
  | 'denied'
  | 'expired'
  | 'consumed'
  | 'authorization_pending'
  | 'slow_down'
  | 'access_denied'

export interface DeviceServerAuthStartResponse {
  device_code: string
  user_code: string
  verification_uri: string
  verification_uri_complete: string
  expires_in: number
  interval: number
}

export interface DeviceServerAuthApprovalResponse {
  user_code: string
  status: ServerAuthStatus
}

export interface PollServerAuthRequest {
  device_code: string
  server_public_key: string
}

export interface DeviceServerAuthPollResponse {
  status: ServerAuthStatus
  interval: number
  credential?: ServerCredentialResponse | null
}

export interface OAuthIdentity {
  provider: 'github'
  provider_user_id: string
  user_id: string
  email: string
  login: string
  avatar_url: string
  created_epoch_sec: number
  updated_epoch_sec: number
}

export interface AdminSessionSummary {
  session_id: string
  user_id: string
  user_email: string
  device_id: string
  device_name: string
  service_id: string
  service_name: string
  client_id: string
  status: 'pending' | 'claimed' | 'bound' | 'closed' | 'expired'
  relay_addr: string
  punch_addr: string
  expire_at: number
}

export interface CreateSessionRequest {
  client_id: string
  device_id: string
  service_id: string
}

export interface CreateSessionResponse {
  session_id: string
  access_token: string
  relay_token: string
  relay_addr: string
  punch_addr: string
  agent_p2p_cert_der?: number[] | null
  expire_at: number
}
