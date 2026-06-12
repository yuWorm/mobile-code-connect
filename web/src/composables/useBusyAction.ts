import { computed, ref } from 'vue'

export function useBusyAction() {
  const busyAction = ref('')
  const hasBusyAction = computed(() => busyAction.value !== '')

  function isBusy(action: string, id: string) {
    return busyAction.value === `${action}:${id}`
  }

  async function runBusyAction<T>(key: string, action: () => Promise<T>) {
    if (busyAction.value !== '') {
      return undefined
    }

    busyAction.value = key
    try {
      return await action()
    } finally {
      if (busyAction.value === key) {
        busyAction.value = ''
      }
    }
  }

  return { busyAction, hasBusyAction, isBusy, runBusyAction }
}
