/**
 * Theme System
 *
 * 提供亮色/暗色主题切换功能
 * 使用 localStorage 持久化用户选择
 */

export type Theme = 'light' | 'dark'

const STORAGE_KEY = 'app-theme'

/**
 * 获取当前主题
 * 优先级：localStorage > 系统偏好 > 默认暗色
 */
export function getTheme(): Theme {
  // 检查 localStorage
  const stored = localStorage.getItem(STORAGE_KEY)
  if (stored === 'light' || stored === 'dark') {
    return stored
  }

  // 检测系统偏好
  if (window.matchMedia) {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    return prefersDark ? 'dark' : 'light'
  }

  // 默认暗色
  return 'dark'
}

/**
 * 设置主题并持久化
 */
export function setTheme(theme: Theme): void {
  localStorage.setItem(STORAGE_KEY, theme)
  document.documentElement.setAttribute('data-theme', theme)
}

/**
 * 切换主题（暗色 <-> 亮色）
 * @returns 切换后的主题
 */
export function toggleTheme(): Theme {
  const current = getTheme()
  const next = current === 'dark' ? 'light' : 'dark'
  setTheme(next)
  return next
}

/**
 * 初始化主题系统
 * 应在应用启动时调用
 */
export function initTheme(): void {
  const theme = getTheme()
  document.documentElement.setAttribute('data-theme', theme)

  // 监听系统主题变化（仅当用户未手动设置时）
  if (window.matchMedia && !localStorage.getItem(STORAGE_KEY)) {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
      const newTheme = e.matches ? 'dark' : 'light'
      setTheme(newTheme)
    })
  }
}
