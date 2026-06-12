import { toast as sonnerToast } from 'vue-sonner'

interface ClipboardWriter {
  writeText(value: string): Promise<void>
}

interface ClipboardToast {
  success(message: string): void
  error(message: string): void
}

interface ClipboardDependencies {
  clipboard: ClipboardWriter
  toast: ClipboardToast
}

interface CopyOptions {
  success?: string
  error?: string
}

export async function copyToClipboard(
  value: string,
  options: CopyOptions = {},
  dependencies?: ClipboardDependencies,
) {
  const clipboard = dependencies?.clipboard ?? navigator.clipboard
  const toast = dependencies?.toast ?? sonnerToast
  const success = options.success ?? '已复制到剪贴板'
  const error = options.error ?? '复制到剪贴板失败'

  try {
    await clipboard.writeText(value)
    toast.success(success)
  } catch (cause) {
    const message = cause instanceof Error ? cause.message : String(cause)
    toast.error(message ? `${error}：${message}` : error)
    throw cause
  }
}
