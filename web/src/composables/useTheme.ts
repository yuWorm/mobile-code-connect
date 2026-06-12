import { ref, watch } from 'vue'

const initialTheme =
  localStorage.getItem('theme') ??
  (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
const isDark = ref(initialTheme === 'dark')

watch(
  isDark,
  (value) => {
    document.documentElement.classList.toggle('dark', value)
    localStorage.setItem('theme', value ? 'dark' : 'light')
  },
  { immediate: true },
)

export function useTheme() {
  return {
    isDark,
    toggleTheme: () => {
      isDark.value = !isDark.value
    },
  }
}
