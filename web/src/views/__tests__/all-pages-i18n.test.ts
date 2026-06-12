import { readdirSync, readFileSync } from 'node:fs'
import { relative, resolve } from 'node:path'

import { describe, expect, test } from 'bun:test'

const viewsDir = resolve(import.meta.dir, '..')

function collectViewFiles(dir = viewsDir): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(dir, entry.name)

    if (entry.isDirectory()) {
      return collectViewFiles(path)
    }

    return entry.isFile() && entry.name.endsWith('.vue')
      ? [relative(viewsDir, path)]
      : []
  }).sort()
}

const viewFiles = collectViewFiles()

describe('route view i18n coverage', () => {
  test('every route view imports and uses the i18n composer', () => {
    for (const file of viewFiles) {
      const source = readFileSync(resolve(viewsDir, file), 'utf8')

      expect(source, file).toContain("import { useI18n } from '@/composables/useI18n'")
      expect(source, file).toContain('useI18n()')
    }
  })

  test('route views do not hard-code PageSection titles or descriptions', () => {
    for (const file of viewFiles) {
      const source = readFileSync(resolve(viewsDir, file), 'utf8')

      expect(source, file).not.toMatch(/<PageSection[^>\n]*\s(?:title|description)="[^"]*"/)
    }
  })

  test('route views do not hard-code common identity labels in templates', () => {
    const hardcodedLabelPatterns = [
      /<InfoRow\s+label="[^"]*"/,
      /<Label[^>]*>[^<{]*(?:ID|Subject|Relay|Punch|Token)[^<{]*<\/Label>/,
      /<SelectItem[^>]*>[^<{]*(?:ID|Subject|Relay|Punch|Token)[^<{]*<\/SelectItem>/,
      /<TableHead[^>]*>[^<{]*(?:ID|Subject|Relay|Punch|Token)[^<{]*<\/TableHead>/,
      /\}\}\s*ID<\/(?:Label|SelectItem|TableHead)>/,
    ]

    for (const file of viewFiles) {
      const source = readFileSync(resolve(viewsDir, file), 'utf8')

      for (const pattern of hardcodedLabelPatterns) {
        expect(source, `${file} ${pattern}`).not.toMatch(pattern)
      }
    }
  })

  test('route views pass the translator into shared label formatters', () => {
    const formatterNames = [
      'formatAuditTargetType',
      'formatCredentialStatus',
      'formatDeviceAuthStatus',
      'formatDeviceStatus',
      'formatEnabledLabel',
      'formatRelayHealth',
      'formatRoleLabel',
      'formatSessionStatus',
    ]

    for (const file of viewFiles) {
      const source = readFileSync(resolve(viewsDir, file), 'utf8')

      for (const formatter of formatterNames) {
        const calls = source.match(new RegExp(`${formatter}\\((?![^)]*,\\s*t\\))`, 'g')) ?? []
        expect(calls, `${file} ${formatter}`).toHaveLength(0)
      }
    }
  })
})
