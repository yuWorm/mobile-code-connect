import { describe, expect, test } from 'bun:test'

import {
  DEFAULT_LOCALE,
  createI18nState,
  isLocale,
  resolveInitialLocale,
} from '../index'
import { messages, type I18nKey, type Locale } from '../messages'

describe('i18n messages', () => {
  test('keeps every locale on the same translation key set', () => {
    const locales = Object.keys(messages) as Locale[]
    const defaultKeys = Object.keys(messages[DEFAULT_LOCALE]).sort()

    for (const locale of locales) {
      expect(Object.keys(messages[locale]).sort()).toEqual(defaultKeys)
    }
  })

  test('recognizes supported locales only', () => {
    expect(isLocale('zh-CN')).toBe(true)
    expect(isLocale('en-US')).toBe(true)
    expect(isLocale('zh')).toBe(false)
  })
})

describe('i18n state', () => {
  test('translates keys with default fallback and parameter interpolation', () => {
    const state = createI18nState('en-US')

    expect(state.t('shell.workspace.admin')).toBe('Admin Console')
    expect(state.t('dashboard.admin.usersDescription', { enabled: 3, admins: 1 })).toBe('3 enabled, 1 admin')
    expect(state.t('missing.key' as I18nKey)).toBe('missing.key')

    state.setLocale('zh-CN')
    expect(state.t('shell.workspace.admin')).toBe('管理后台')
  })

  test('resolves the initial locale from storage and persists changes', () => {
    const storage = new Map<string, string>()
    const adapter = {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    }

    adapter.setItem('control.locale', 'en-US')
    expect(resolveInitialLocale(adapter)).toBe('en-US')
    adapter.setItem('control.locale', 'fr-FR')
    expect(resolveInitialLocale(adapter)).toBe(DEFAULT_LOCALE)

    const state = createI18nState('zh-CN', adapter)
    state.setLocale('en-US')
    expect(storage.get('control.locale')).toBe('en-US')
  })
})
