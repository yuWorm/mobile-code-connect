import { describe, expect, test } from 'bun:test'

import { useAsyncData } from '@/composables/useAsyncData'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })

  return { promise, reject, resolve }
}

describe('useAsyncData', () => {
  test('keeps the newest refresh result when earlier requests finish later', async () => {
    const first = deferred<string>()
    const second = deferred<string>()
    const requests = [first.promise, second.promise]
    const state = useAsyncData(() => requests.shift()!, false)

    const firstRefresh = state.refresh()
    const secondRefresh = state.refresh()

    second.resolve('newest')
    await secondRefresh
    expect(state.data.value).toBe('newest')
    expect(state.loading.value).toBe(false)

    first.resolve('stale')
    await firstRefresh
    expect(state.data.value).toBe('newest')
    expect(state.error.value).toBe('')
  })

  test('ignores stale failures and keeps loading until the newest refresh settles', async () => {
    const first = deferred<string>()
    const second = deferred<string>()
    const requests = [first.promise, second.promise]
    const state = useAsyncData(() => requests.shift()!, false)

    const firstRefresh = state.refresh()
    const secondRefresh = state.refresh()

    first.reject(new Error('older request failed'))
    await firstRefresh
    expect(state.loading.value).toBe(true)
    expect(state.error.value).toBe('')
    expect(state.data.value).toBeNull()

    second.resolve('fresh')
    await secondRefresh
    expect(state.loading.value).toBe(false)
    expect(state.error.value).toBe('')
    expect(state.data.value).toBe('fresh')
  })
})
