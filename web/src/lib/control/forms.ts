import type {
  AssignUserPlanRequest,
  CreateSessionRequest,
  GrantDeviceAccessRequest,
  UpdateUserPlanRequest,
} from './types'

interface AccessGrantForm {
  user_id: string
}

interface UserPlanAssignmentForm {
  plan_id: string
}

interface UserPlanUpdateForm {
  plan_id: string
  name: string
  max_controller_devices: number | string
  relay_limits: {
    max_bps: number | string
    max_streams: number | string
    max_duration_sec: number | string
    traffic_quota_bytes: number | string
  }
}

interface SessionForm {
  client_id: string
  device_id: string
  service_id: string
}

export function createAccessGrantRequest(form: AccessGrantForm): GrantDeviceAccessRequest {
  return {
    user_id: form.user_id.trim(),
  }
}

export function createUserPlanAssignmentRequest(
  form: UserPlanAssignmentForm,
): AssignUserPlanRequest {
  return {
    plan_id: form.plan_id.trim(),
  }
}

export function createUserPlanUpdateRequest(form: UserPlanUpdateForm): UpdateUserPlanRequest {
  return {
    plan: {
      plan_id: form.plan_id.trim(),
      name: form.name.trim(),
      max_controller_devices: Number(form.max_controller_devices),
      relay_limits: {
        max_bps: Number(form.relay_limits.max_bps),
        max_streams: Number(form.relay_limits.max_streams),
        max_duration_sec: Number(form.relay_limits.max_duration_sec),
        traffic_quota_bytes: Number(form.relay_limits.traffic_quota_bytes),
      },
    },
  }
}

export function createSessionRequest(form: SessionForm): CreateSessionRequest {
  return {
    client_id: form.client_id.trim(),
    device_id: form.device_id.trim(),
    service_id: form.service_id.trim(),
  }
}
