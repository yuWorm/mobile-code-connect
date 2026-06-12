import { describe, expect, test } from 'bun:test'

import { runWithToast } from '../action'

describe('runWithToast', () => {
  test('runs an operation and shows success feedback', async () => {
    const messages: string[] = []

    const result = await runWithToast(
      async () => 'ok',
      {
        success: '操作成功',
        error: '操作失败',
      },
      {
        success: (message) => messages.push(`success:${message}`),
        error: (message) => messages.push(`error:${message}`),
      },
    )

    expect(result).toBe('ok')
    expect(messages).toEqual(['success:操作成功'])
  })

  test('maps thrown errors into failure feedback and rethrows', async () => {
    const messages: string[] = []

    await expect(
      runWithToast(
        async () => {
          throw new Error('backend unavailable')
        },
        {
          success: '操作成功',
          error: '操作失败',
        },
        {
          success: (message) => messages.push(`success:${message}`),
          error: (message) => messages.push(`error:${message}`),
        },
      ),
    ).rejects.toThrow('backend unavailable')

    expect(messages).toEqual(['error:操作失败：backend unavailable'])
  })

  test('allows success feedback to be selected from the result', async () => {
    const messages: string[] = []

    await runWithToast(
      async () => 'pending',
      {
        success: (result) => (result === 'issued' ? '已签发' : ''),
        error: '操作失败',
      },
      {
        success: (message) => messages.push(`success:${message}`),
        error: (message) => messages.push(`error:${message}`),
      },
    )

    expect(messages).toEqual([])
  })
})
