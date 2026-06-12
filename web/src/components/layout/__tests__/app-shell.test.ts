import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'
import { parse } from '@vue/compiler-sfc'

function appShellParts() {
  const source = readFileSync(new URL('../AppShell.vue', import.meta.url), 'utf8')
  const { descriptor, errors } = parse(source, { filename: 'AppShell.vue' })

  expect(errors).toEqual([])
  expect(descriptor.scriptSetup).not.toBeNull()
  expect(descriptor.template).not.toBeNull()

  return {
    script: descriptor.scriptSetup?.content ?? '',
    template: descriptor.template?.content ?? '',
  }
}

function lucideImports(script: string) {
  const match = script.match(/import\s+\{(?<imports>[^}]+)\}\s+from\s+'lucide-vue-next'/)
  return new Set(
    match?.groups?.imports
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean) ?? [],
  )
}

describe('AppShell', () => {
  test('imports the lucide icons it renders directly', () => {
    const { script, template } = appShellParts()
    const imports = lucideImports(script)

    for (const icon of ['LogOut', 'Menu', 'Moon', 'ShieldCheck', 'Sun', 'Users', 'X']) {
      expect(template).toContain(`<${icon}`)
      expect(imports.has(icon)).toBe(true)
    }
  })

  test('labels icon-only shell actions for assistive technology', () => {
    const { template } = appShellParts()

    expect(template).toContain(':aria-label="t(\'shell.openNav\')"')
    expect(template).toContain(':aria-label="t(\'shell.toggleTheme\')"')
    expect(template).toContain(':aria-label="t(\'shell.logout\')"')
    expect(template).toContain(':aria-label="t(\'shell.closeNav\')"')
  })

  test('renders translated navigation and a language switcher', () => {
    const { script, template } = appShellParts()

    expect(script).toContain("import { useI18n } from '@/composables/useI18n'")
    expect(script).toContain('const { locale, locales, setLocale, t } = useI18n()')
    expect(script).toContain("route.meta.titleKey")
    expect(template).toContain('{{ t(item.labelKey) }}')
    expect(template).toContain('<Select :model-value="locale" @update:model-value="setLocale(String($event))">')
    expect(template).toContain(':aria-label="t(\'shell.language\')"')
    expect(template).toContain('locale.label')
    expect(template).toContain('formatRoleLabel(state.session?.role, t)')
    expect(template).not.toContain('t(formatRoleLabel')
  })
})
