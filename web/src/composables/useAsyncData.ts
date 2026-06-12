import { onMounted, ref } from 'vue'

import { controlApiErrorMessage } from '@/lib/control/api'

export function useAsyncData<T>(loader: () => Promise<T>, immediate = true) {
  const data = ref<T | null>(null)
  const loading = ref(false)
  const error = ref('')
  let refreshId = 0

  async function refresh() {
    const currentRefreshId = ++refreshId
    loading.value = true
    error.value = ''
    try {
      const nextData = await loader()
      if (currentRefreshId === refreshId) {
        data.value = nextData
      }
    } catch (cause) {
      if (currentRefreshId === refreshId) {
        error.value = controlApiErrorMessage(cause, {
          unauthorized: '登录已失效，请重新登录',
          forbidden: '当前账号没有权限访问此数据',
          fallback: '请求失败',
        })
      }
    } finally {
      if (currentRefreshId === refreshId) {
        loading.value = false
      }
    }
  }

  if (immediate) {
    onMounted(refresh)
  }

  return { data, loading, error, refresh }
}
