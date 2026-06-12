import {
  Activity,
  Gauge,
  HardDrive,
  KeyRound,
  LayoutDashboard,
  Network,
  Shield,
  ScrollText,
  Server,
  ShieldCheck,
  Users,
  WalletCards,
} from 'lucide-vue-next'

export const adminNavItems = [
  { label: '总览', to: '/admin', labelKey: 'nav.admin.dashboard', icon: LayoutDashboard },
  { label: '用户', to: '/admin/users', labelKey: 'nav.admin.users', icon: Users },
  { label: '设备', to: '/admin/devices', labelKey: 'nav.admin.devices', icon: HardDrive },
  { label: '会话', to: '/admin/sessions', labelKey: 'nav.admin.sessions', icon: Activity },
  { label: 'Relay', to: '/admin/relays', labelKey: 'nav.admin.relays', icon: Network },
  { label: '服务凭据', to: '/admin/credentials', labelKey: 'nav.admin.credentials', icon: Server },
  { label: '套餐', to: '/admin/plans', labelKey: 'nav.admin.plans', icon: WalletCards },
  { label: 'OAuth', to: '/admin/oauth', labelKey: 'nav.admin.oauth', icon: Shield },
  { label: '审计', to: '/admin/audit', labelKey: 'nav.admin.audit', icon: ScrollText },
] as const

export const centerNavItems = [
  { label: '中台总览', to: '/center', labelKey: 'nav.center.dashboard', icon: Gauge },
  { label: '我的设备', to: '/center/devices', labelKey: 'nav.center.devices', icon: HardDrive },
  { label: '控制器', to: '/center/controllers', labelKey: 'nav.center.controllers', icon: ShieldCheck },
  { label: '服务凭据', to: '/center/credentials', labelKey: 'nav.center.credentials', icon: Server },
  { label: '账号安全', to: '/center/account', labelKey: 'nav.center.account', icon: KeyRound },
] as const
