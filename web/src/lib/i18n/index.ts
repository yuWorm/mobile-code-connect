import { readonly, ref } from 'vue'

import { messages, type I18nKey, type Locale } from './messages'

export const DEFAULT_LOCALE: Locale = 'zh-CN'
export const LOCALE_STORAGE_KEY = 'control.locale'

export interface StorageLike {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

export const localeOptions: Array<{ value: Locale; label: string }> = [
  { value: 'zh-CN', label: '简体中文' },
  { value: 'en-US', label: 'English' },
]

export function isLocale(value: string): value is Locale {
  return Object.prototype.hasOwnProperty.call(messages, value)
}

function browserStorage(): StorageLike | undefined {
  if (typeof window === 'undefined') {
    return undefined
  }
  return window.localStorage
}

export function resolveInitialLocale(storage: StorageLike | undefined = browserStorage()): Locale {
  const stored = storage?.getItem(LOCALE_STORAGE_KEY)
  return stored && isLocale(stored) ? stored : DEFAULT_LOCALE
}

function interpolate(message: string, params?: Record<string, string | number>) {
  if (!params) {
    return message
  }
  return message.replace(/\{(\w+)\}/g, (_, key: string) => String(params[key] ?? `{${key}}`))
}

export function createI18nState(initialLocale = resolveInitialLocale(), storage: StorageLike | undefined = browserStorage()) {
  const locale = ref<Locale>(isLocale(initialLocale) ? initialLocale : DEFAULT_LOCALE)

  function setLocale(value: string) {
    if (!isLocale(value)) {
      return
    }
    locale.value = value
    storage?.setItem(LOCALE_STORAGE_KEY, value)
  }

  function t(key: I18nKey | string, params?: Record<string, string | number>) {
    const messageKey = key as I18nKey
    const localeMessage = messages[locale.value][messageKey]
    const fallbackMessage = messages[DEFAULT_LOCALE][messageKey]
    return interpolate(localeMessage ?? fallbackMessage ?? key, params)
  }

  return {
    locale,
    locales: readonly(localeOptions),
    setLocale,
    t,
  }
}

export const i18n = createI18nState()
