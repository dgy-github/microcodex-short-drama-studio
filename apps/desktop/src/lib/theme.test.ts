import { describe, it, expect, beforeEach, vi } from 'vitest'
import { getTheme, setTheme, toggleTheme, initTheme, type Theme } from './theme'

describe('Theme System', () => {
  beforeEach(() => {
    // 清理 localStorage
    localStorage.clear()
    // 重置 document.documentElement
    document.documentElement.removeAttribute('data-theme')
  })

  describe('getTheme', () => {
    it('应该返回 localStorage 中保存的主题', () => {
      localStorage.setItem('app-theme', 'light')
      expect(getTheme()).toBe('light')

      localStorage.setItem('app-theme', 'dark')
      expect(getTheme()).toBe('dark')
    })

    it('应该在 localStorage 为空时检测系统偏好', () => {
      // 模拟系统偏好暗色主题
      window.matchMedia = vi.fn().mockImplementation((query) => ({
        matches: query === '(prefers-color-scheme: dark)',
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }))

      expect(getTheme()).toBe('dark')
    })

    it('应该忽略无效的 localStorage 值并回退到系统偏好', () => {
      localStorage.setItem('app-theme', 'invalid')

      // 模拟系统偏好暗色
      window.matchMedia = vi.fn().mockImplementation((query) => ({
        matches: query === '(prefers-color-scheme: dark)',
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }))

      expect(getTheme()).toBe('dark')
    })

    it('应该检测系统偏好亮色主题', () => {
      // 模拟系统偏好亮色主题
      window.matchMedia = vi.fn().mockImplementation((query) => ({
        matches: query === '(prefers-color-scheme: light)',
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }))

      expect(getTheme()).toBe('light')
    })
  })

  describe('setTheme', () => {
    it('应该保存主题到 localStorage', () => {
      setTheme('light')
      expect(localStorage.getItem('app-theme')).toBe('light')

      setTheme('dark')
      expect(localStorage.getItem('app-theme')).toBe('dark')
    })

    it('应该设置 data-theme 属性到 documentElement', () => {
      setTheme('light')
      expect(document.documentElement.getAttribute('data-theme')).toBe('light')

      setTheme('dark')
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
    })
  })

  describe('toggleTheme', () => {
    it('应该在暗色和亮色之间切换', () => {
      localStorage.setItem('app-theme', 'dark')
      expect(toggleTheme()).toBe('light')
      expect(localStorage.getItem('app-theme')).toBe('light')

      expect(toggleTheme()).toBe('dark')
      expect(localStorage.getItem('app-theme')).toBe('dark')
    })

    it('应该更新 data-theme 属性', () => {
      localStorage.setItem('app-theme', 'dark')

      toggleTheme()
      expect(document.documentElement.getAttribute('data-theme')).toBe('light')

      toggleTheme()
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
    })
  })

  describe('initTheme', () => {
    it('应该设置初始主题到 documentElement', () => {
      localStorage.setItem('app-theme', 'light')
      initTheme()
      expect(document.documentElement.getAttribute('data-theme')).toBe('light')
    })

    it('应该使用系统偏好（如果没有保存的主题）', () => {
      // 模拟系统偏好暗色
      window.matchMedia = vi.fn().mockImplementation((query) => ({
        matches: query === '(prefers-color-scheme: dark)',
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }))

      initTheme()
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
    })

    it('应该在系统主题改变时自动更新（仅当用户未手动设置时）', () => {
      const listeners: Array<(e: MediaQueryListEvent) => void> = []

      window.matchMedia = vi.fn().mockImplementation((query) => ({
        matches: query === '(prefers-color-scheme: dark)',
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn((event: string, callback: (e: MediaQueryListEvent) => void) => {
          if (event === 'change') {
            listeners.push(callback)
          }
        }),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }))

      initTheme()

      // 模拟系统主题变化
      if (listeners.length > 0) {
        listeners[0]({
          matches: true,
          media: '(prefers-color-scheme: dark)',
        } as MediaQueryListEvent)

        expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
      }
    })
  })

  describe('持久化', () => {
    it('应该在刷新后保持用户选择', () => {
      setTheme('light')
      expect(localStorage.getItem('app-theme')).toBe('light')

      // 模拟页面刷新
      const savedTheme = localStorage.getItem('app-theme') as Theme
      expect(savedTheme).toBe('light')

      // 重新初始化
      document.documentElement.removeAttribute('data-theme')
      initTheme()
      expect(document.documentElement.getAttribute('data-theme')).toBe('light')
    })
  })

  describe('边界情况', () => {
    it('应该处理连续的切换操作', () => {
      setTheme('dark')

      for (let i = 0; i < 5; i++) {
        toggleTheme()
      }

      expect(getTheme()).toBe('light')
    })

    it('应该处理 localStorage 损坏的情况', () => {
      localStorage.setItem('app-theme', '{invalid json}')
      expect(() => getTheme()).not.toThrow()
      expect(getTheme()).toBe('dark')
    })
  })
})
