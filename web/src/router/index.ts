import { createRouter, createWebHistory } from 'vue-router'

import { clearStoredSession, readStoredSession } from '@/lib/control/auth'
import { resolveHomeRedirect, resolveRouteGuard } from './guards'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      redirect: () => {
        return resolveHomeRedirect(readStoredSession(), { clearSession: clearStoredSession })
      },
    },
    {
      path: '/login',
      name: 'login',
      component: () => import('@/views/LoginView.vue'),
      meta: {
        public: true,
        title: '登录',
        titleKey: 'route.login.title',
      },
    },
    {
      path: '/login/oauth/github/callback',
      name: 'oauth-github-callback',
      component: () => import('@/views/OAuthGithubCallbackView.vue'),
      meta: {
        public: true,
        title: 'GitHub 登录',
        titleKey: 'route.oauthGithub.title',
      },
    },
    {
      path: '/admin',
      component: () => import('@/components/layout/AppShell.vue'),
      meta: {
        requiresAuth: true,
        requiresAdmin: true,
      },
      children: [
        {
          path: '',
          name: 'admin-dashboard',
          component: () => import('@/views/admin/AdminDashboardView.vue'),
          meta: { title: '管理总览', titleKey: 'route.admin.dashboard.title', description: '用户、设备、会话和 Relay 的全局状态', descriptionKey: 'route.admin.dashboard.description' },
        },
        {
          path: 'users',
          name: 'admin-users',
          component: () => import('@/views/admin/AdminUsersView.vue'),
          meta: { title: '用户管理', titleKey: 'route.admin.users.title', description: '创建用户、调整状态、角色和套餐', descriptionKey: 'route.admin.users.description' },
        },
        {
          path: 'devices',
          name: 'admin-devices',
          component: () => import('@/views/admin/AdminDevicesView.vue'),
          meta: { title: '设备管理', titleKey: 'route.admin.devices.title', description: '查看受控设备并管理访问授权', descriptionKey: 'route.admin.devices.description' },
        },
        {
          path: 'sessions',
          name: 'admin-sessions',
          component: () => import('@/views/admin/AdminSessionsView.vue'),
          meta: { title: '会话管理', titleKey: 'route.admin.sessions.title', description: '查看和关闭控制会话', descriptionKey: 'route.admin.sessions.description' },
        },
        {
          path: 'relays',
          name: 'admin-relays',
          component: () => import('@/views/admin/AdminRelaysView.vue'),
          meta: { title: 'Relay 管理', titleKey: 'route.admin.relays.title', description: 'Relay 节点和凭据生命周期', descriptionKey: 'route.admin.relays.description' },
        },
        {
          path: 'credentials',
          name: 'admin-credentials',
          component: () => import('@/views/admin/AdminCredentialsView.vue'),
          meta: { title: '服务凭据', titleKey: 'route.admin.credentials.title', description: '全局管理受控服务器登录凭据', descriptionKey: 'route.admin.credentials.description' },
        },
        {
          path: 'plans',
          name: 'admin-plans',
          component: () => import('@/views/admin/AdminPlansView.vue'),
          meta: { title: '套餐管理', titleKey: 'route.admin.plans.title', description: '计划模板和用户套餐', descriptionKey: 'route.admin.plans.description' },
        },
        {
          path: 'oauth',
          name: 'admin-oauth',
          component: () => import('@/views/admin/AdminOAuthView.vue'),
          meta: { title: 'OAuth 身份', titleKey: 'route.admin.oauth.title', description: '第三方登录身份关联和解绑', descriptionKey: 'route.admin.oauth.description' },
        },
        {
          path: 'audit',
          name: 'admin-audit',
          component: () => import('@/views/admin/AdminAuditView.vue'),
          meta: { title: '审计日志', titleKey: 'route.admin.audit.title', description: '管理动作和关键变更记录', descriptionKey: 'route.admin.audit.description' },
        },
      ],
    },
    {
      path: '/center',
      component: () => import('@/components/layout/AppShell.vue'),
      meta: {
        requiresAuth: true,
      },
      children: [
        {
          path: '',
          name: 'center-dashboard',
          component: () => import('@/views/user/UserDashboardView.vue'),
          meta: { title: '中台总览', titleKey: 'route.center.dashboard.title', description: '我的计划、设备、凭据和控制入口', descriptionKey: 'route.center.dashboard.description' },
        },
        {
          path: 'devices',
          name: 'center-devices',
          component: () => import('@/views/user/UserDevicesView.vue'),
          meta: { title: '我的设备', titleKey: 'route.center.devices.title', description: '查看可访问设备和服务', descriptionKey: 'route.center.devices.description' },
        },
        {
          path: 'controllers',
          name: 'center-controllers',
          component: () => import('@/views/user/UserControllersView.vue'),
          meta: { title: '控制器', titleKey: 'route.center.controllers.title', description: '管理客户端控制器身份', descriptionKey: 'route.center.controllers.description' },
        },
        {
          path: 'credentials',
          name: 'center-credentials',
          component: () => import('@/views/user/UserCredentialsView.vue'),
          meta: { title: '服务凭据', titleKey: 'route.center.credentials.title', description: '管理受控服务器登录凭据', descriptionKey: 'route.center.credentials.description' },
        },
        {
          path: 'account',
          name: 'center-account',
          component: () => import('@/views/user/UserAccountView.vue'),
          meta: { title: '账号安全', titleKey: 'route.center.account.title', description: '密码和 OAuth 身份', descriptionKey: 'route.center.account.description' },
        },
      ],
    },
  ],
})

router.beforeEach((to) => resolveRouteGuard(to, readStoredSession(), { clearSession: clearStoredSession }))

export default router
