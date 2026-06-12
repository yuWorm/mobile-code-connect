import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'
import { compileTemplate, parse } from '@vue/compiler-sfc'

function compiledTemplateFor(path: string) {
  const source = readFileSync(new URL(path, import.meta.url), 'utf8')
  const { descriptor, errors } = parse(source, { filename: path })

  expect(errors).toEqual([])
  expect(descriptor.template).not.toBeNull()

  return compileTemplate({
    source: descriptor.template?.content ?? '',
    filename: path,
    id: path,
  }).code
}

describe('form controls', () => {
  test('Input forwards component v-model to the native input element', () => {
    expect(compiledTemplateFor('../input/Input.vue')).toContain('vModelText')
  })

  test('Textarea forwards component v-model to the native textarea element', () => {
    expect(compiledTemplateFor('../textarea/Textarea.vue')).toContain('vModelText')
  })

  test('Button defaults to a non-submit native button type', () => {
    const source = readFileSync(new URL('../button/Button.vue', import.meta.url), 'utf8')

    expect(source).toContain("type?: 'button' | 'submit' | 'reset'")
    expect(source).toContain("type: 'button'")
    expect(source).toContain(':type="props.as === \'button\' ? type : undefined"')
  })
})
