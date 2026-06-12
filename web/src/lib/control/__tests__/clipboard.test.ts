import { describe, expect, test } from 'bun:test'

import { copyToClipboard } from '../clipboard'

describe('copyToClipboard', () => {
  test('writes text and shows success feedback', async () => {
    const writes: string[] = []
    const messages: string[] = []

    await copyToClipboard(
      'server-token',
      {},
      {
        clipboard: {
          writeText: async (value) => {
            writes.push(value)
          },
        },
        toast: {
          success: (message) => messages.push(`success:${message}`),
          error: (message) => messages.push(`error:${message}`),
        },
      },
    )

    expect(writes).toEqual(['server-token'])
    expect(messages).toEqual(['success:已复制到剪贴板'])
  })

  test('shows failure feedback and rethrows clipboard errors', async () => {
    const messages: string[] = []

    await expect(
      copyToClipboard(
        'server-token',
        {},
        {
          clipboard: {
            writeText: async () => {
              throw new Error('permission denied')
            },
          },
          toast: {
            success: (message) => messages.push(`success:${message}`),
            error: (message) => messages.push(`error:${message}`),
          },
        },
      ),
    ).rejects.toThrow('permission denied')

    expect(messages).toEqual(['error:复制到剪贴板失败：permission denied'])
  })
})
