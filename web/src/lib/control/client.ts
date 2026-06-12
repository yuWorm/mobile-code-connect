import { ControlApi } from './api'
import type { ControlApiAuthFailureHandler } from './api'
import { readStoredSession } from './auth'

const defaultBaseUrl = import.meta.env.VITE_CONTROL_API_BASE_URL ?? ''

export const controlApi = new ControlApi({
  baseUrl: defaultBaseUrl,
  token: readStoredSession()?.accessToken ?? null,
})

export function setControlApiToken(token: string | null) {
  controlApi.setToken(token)
}

export function setControlApiAuthFailureHandler(handler: ControlApiAuthFailureHandler | null) {
  controlApi.setAuthFailureHandler(handler)
}

export function setControlApiBaseUrl(baseUrl: string) {
  controlApi.setBaseUrl(baseUrl)
}
