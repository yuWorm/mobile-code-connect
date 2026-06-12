import { computed, reactive } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { controlApiErrorMessage } from '@/lib/control/api'
import { controlApi, setControlApiAuthFailureHandler, setControlApiToken } from '@/lib/control/client'
import {
  clearStoredSession,
  readStoredSession,
  safeRedirectTarget,
  sessionFromAuthResponse,
  writeStoredSession,
} from '@/lib/control/auth'
import type { AuthSession, LoginRequest, RegisterUserRequest } from '@/lib/control/types'

interface AuthState {
  session: AuthSession | null
  loading: boolean
  error: string
}

const state = reactive<AuthState>({
  session: readStoredSession(),
  loading: false,
  error: '',
})

setControlApiToken(state.session?.accessToken ?? null)

export function useAuth() {
  const route = useRoute()
  const router = useRouter()
  const isAuthenticated = computed(() => Boolean(state.session?.accessToken))
  const isAdmin = computed(() => state.session?.role === 'admin')

  setControlApiAuthFailureHandler(handleAuthFailure)

  async function login(request: LoginRequest) {
    state.loading = true
    state.error = ''
    try {
      const response = await controlApi.login(request)
      setSession(sessionFromAuthResponse(response))
      await router.replace(safeRedirectTarget(route.query.redirect, state.session?.role))
    } catch (error) {
      state.error = controlApiErrorMessage(error, {
        unauthorized: '邮箱或密码错误，请检查后重试',
        forbidden: '当前账号无权登录控制台',
        fallback: '登录失败',
      })
    } finally {
      state.loading = false
    }
  }

  async function register(request: RegisterUserRequest) {
    state.loading = true
    state.error = ''
    try {
      const response = await controlApi.register(request)
      setSession(sessionFromAuthResponse(response))
      await router.replace(safeRedirectTarget(route.query.redirect, state.session?.role))
    } catch (error) {
      state.error = controlApiErrorMessage(error, {
        unauthorized: '无法注册账号，请检查服务端认证配置',
        forbidden: '当前服务端不允许注册账号',
        fallback: '注册失败',
      })
    } finally {
      state.loading = false
    }
  }

  async function logout() {
    clearStoredSession()
    setControlApiToken(null)
    state.session = null
    await router.push('/login')
  }

  function setSession(session: AuthSession) {
    state.session = session
    writeStoredSession(session)
    setControlApiToken(session.accessToken)
  }

  function handleAuthFailure() {
    if (!state.session) {
      return
    }
    clearStoredSession()
    setControlApiToken(null)
    state.session = null

    const currentRoute = router.currentRoute.value
    if (currentRoute.path !== '/login') {
      void router.replace({ path: '/login', query: { redirect: currentRoute.fullPath } })
    }
  }

  return {
    state,
    isAuthenticated,
    isAdmin,
    login,
    logout,
    register,
    setSession,
  }
}
