import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

import { messages } from '@/lib/i18n/messages'

describe('login page design', () => {
  test('uses the MobileCode Connect product name and logo key', () => {
    const loginSource = readFileSync(new URL('../LoginView.vue', import.meta.url), 'utf8')
    const shellSource = readFileSync(new URL('../../components/layout/AppShell.vue', import.meta.url), 'utf8')

    expect(messages['zh-CN']['app.name']).toBe('MobileCode Connect')
    expect(messages['en-US']['app.name']).toBe('MobileCode Connect')
    expect(messages['zh-CN']['app.logo']).toBe('MC')
    expect(messages['en-US']['app.logo']).toBe('MC')
    expect(loginSource).toContain("{{ t('app.logo') }}")
    expect(shellSource).toContain("{{ t('app.logo') }}")
    expect(loginSource).not.toContain('QT')
    expect(shellSource).not.toContain('QT')
  })

  test('keeps the login experience visually balanced across desktop and mobile', () => {
    const source = readFileSync(new URL('../LoginView.vue', import.meta.url), 'utf8')

    expect(source).toContain('lg:grid-cols-2')
    expect(source).toContain('absolute inset-x-0 top-0 z-20')
    expect(source).toContain('overflow-x-hidden')
    expect(source).toContain('absolute right-5 top-5 sm:right-8 lg:right-12 lg:top-8')
    expect(source).toContain('max-w-[8.5rem] truncate sm:max-w-none')
    expect(source).toContain('login-brand-panel')
    expect(source).toContain('order-2')
    expect(source).toContain('lg:order-1')
    expect(source).toContain('order-1')
    expect(source).toContain('lg:order-2')
    expect(source).toContain('mx-auto flex w-full max-w-xl flex-col items-center justify-center')
    expect(source).toContain('text-center')
    expect(source).toContain("t('auth.heroEyebrow')")
    expect(source).toContain("t('auth.heroSlogan')")
    expect(source).toContain('relative mt-10 h-64 w-full max-w-md')
    expect(source).toContain('absolute left-1/2 top-1/2 flex size-24')
    expect(source).toContain('w-full max-w-[20rem] sm:max-w-[28rem]')
    expect(source).toContain("t('auth.formEyebrow')")
    expect(source).not.toContain('Card class=')
    expect(source).not.toContain('CardHeader')
    expect(source).not.toContain('lg:grid-cols-[minmax(0,1.05fr)_minmax(420px,0.95fr)]')
    expect(source).not.toContain('lg:grid-cols-[1fr_480px]')
    expect(source).not.toContain('hidden min-h-screen flex-col justify-between border-r')
  })

  test('ships localized slogan copy for the branded login panel', () => {
    expect(messages['zh-CN']['auth.heroEyebrow']).toBe('SECURE DEVICE ACCESS')
    expect(messages['zh-CN']['auth.heroSlogan']).toBe('连接移动设备、Relay 与服务凭据的一站式入口')
    expect(messages['zh-CN']['auth.formEyebrow']).toBe('CONTROL ACCESS')
    expect(messages['en-US']['auth.heroEyebrow']).toBe('SECURE DEVICE ACCESS')
    expect(messages['en-US']['auth.heroSlogan']).toBe('One entry for mobile devices, Relay, and service credentials')
    expect(messages['en-US']['auth.formEyebrow']).toBe('CONTROL ACCESS')
  })
})
