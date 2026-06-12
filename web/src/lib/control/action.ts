import { toast } from 'vue-sonner'

import { controlApiErrorMessage } from './api'

export interface ActionToastOptions<T = unknown> {
  success: string | ((result: T) => string)
  error: string
}

export interface ActionNotifier {
  success(message: string): void
  error(message: string): void
}

const defaultNotifier: ActionNotifier = {
  success: (message) => toast.success(message),
  error: (message) => toast.error(message),
}

export async function runWithToast<T>(
  operation: () => Promise<T>,
  messages: ActionToastOptions<T>,
  notifier: ActionNotifier = defaultNotifier,
) {
  try {
    const result = await operation()
    const successMessage =
      typeof messages.success === 'function' ? messages.success(result) : messages.success
    if (successMessage) {
      notifier.success(successMessage)
    }
    return result
  } catch (error) {
    notifier.error(`${messages.error}：${controlApiErrorMessage(error)}`)
    throw error
  }
}
