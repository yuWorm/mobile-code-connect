import { describe, expect, test } from 'bun:test'

import { useBusyAction } from '@/composables/useBusyAction'

describe('useBusyAction', () => {
  test('runs one exclusive action at a time and exposes the active key', async () => {
    const { hasBusyAction, isBusy, runBusyAction } = useBusyAction()
    let releaseAction!: () => void
    let calls = 0

    const first = runBusyAction('rotate:item-a', async () => {
      calls += 1
      await new Promise<void>((resolve) => {
        releaseAction = resolve
      })
      return 'rotated'
    })

    expect(hasBusyAction.value).toBe(true)
    expect(isBusy('rotate', 'item-a')).toBe(true)

    const skipped = await runBusyAction('toggle:item-b', async () => {
      calls += 1
      return 'toggled'
    })

    expect(skipped).toBeUndefined()
    expect(calls).toBe(1)

    releaseAction()
    expect(await first).toBe('rotated')
    expect(hasBusyAction.value).toBe(false)
    expect(isBusy('rotate', 'item-a')).toBe(false)
  })
})
