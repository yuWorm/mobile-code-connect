import { readdirSync, readFileSync } from 'node:fs'
import { relative, resolve } from 'node:path'

import { describe, expect, test } from 'bun:test'

const scanRoots = [
  resolve(import.meta.dir, '..'),
  resolve(import.meta.dir, '../../components'),
]

function collectVueFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(dir, entry.name)

    if (entry.isDirectory()) {
      return collectVueFiles(path)
    }

    return entry.isFile() && entry.name.endsWith('.vue')
      ? [path]
      : []
  }).sort()
}

describe('select item values', () => {
  test('do not render empty string SelectItem values', () => {
    for (const root of scanRoots) {
      for (const file of collectVueFiles(root)) {
        const source = readFileSync(file, 'utf8')

        expect(source, relative(resolve(import.meta.dir, '../..'), file)).not.toContain('<SelectItem value="">')
      }
    }
  })
})
