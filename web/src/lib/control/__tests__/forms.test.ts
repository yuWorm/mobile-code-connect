import { describe, expect, test } from 'bun:test'

import {
  createAccessGrantRequest,
  createSessionRequest,
  createUserPlanAssignmentRequest,
  createUserPlanUpdateRequest,
} from '../forms'

describe('control form payload helpers', () => {
  test('trims user ids before granting device access', () => {
    expect(createAccessGrantRequest({ user_id: '  user_001  ' })).toEqual({
      user_id: 'user_001',
    })
  })

  test('builds a plan assignment request from a selected plan id', () => {
    expect(createUserPlanAssignmentRequest({ plan_id: '  pro  ' })).toEqual({
      plan_id: 'pro',
    })
  })

  test('builds a user plan override request with numeric limits', () => {
    expect(
      createUserPlanUpdateRequest({
        plan_id: '  custom-pro  ',
        name: ' Custom Pro ',
        max_controller_devices: '4',
        relay_limits: {
          max_bps: '2048',
          max_streams: '16',
          max_duration_sec: '7200',
          traffic_quota_bytes: '4096',
        },
      }),
    ).toEqual({
      plan: {
        plan_id: 'custom-pro',
        name: 'Custom Pro',
        max_controller_devices: 4,
        relay_limits: {
          max_bps: 2048,
          max_streams: 16,
          max_duration_sec: 7200,
          traffic_quota_bytes: 4096,
        },
      },
    })
  })

  test('builds a session request from selected controller and service ids', () => {
    expect(
      createSessionRequest({
        client_id: ' phone_1 ',
        device_id: ' dev_1 ',
        service_id: ' ssh ',
      }),
    ).toEqual({
      client_id: 'phone_1',
      device_id: 'dev_1',
      service_id: 'ssh',
    })
  })
})
