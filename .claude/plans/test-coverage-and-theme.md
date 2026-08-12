# 测试覆盖 + 主题切换实施计划

## 📋 总览

**目标 1**: 为桌面应用添加测试覆盖（Rust 后端 + 前端）  
**目标 2**: 实现亮色/暗色主题切换功能

**预计时间**: 7-10 小时
- 测试覆盖: 6-8 小时
- 主题切换: 1-2 小时

---

## 🎯 目标 1: 测试覆盖

### 现状分析

**已有测试**:
- ✅ Rust 单元测试框架已配置
- ✅ 部分模块有基础测试（artifacts.rs, commands.rs 等）
- ✅ GitLab CI 已配置 `cargo test`

**缺失**:
- ❌ 前端组件测试（Svelte）
- ❌ 集成测试
- ❌ E2E 测试
- ❌ 测试覆盖率报告

### 实施策略

#### A. Rust 后端测试增强 (3-4h)

**1. 核心业务逻辑单元测试**

覆盖模块：
- `crates/story-storage/src/export_formats.rs` - 多格式导出
- `crates/story-storage/src/revisions.rs` - 版本管理
- `apps/desktop/src-tauri/src/artifacts.rs` - 作品管理
- `apps/desktop/src-tauri/src/revisions.rs` - 修订服务

测试内容：
```rust
// export_formats.rs 测试
#[cfg(test)]
mod tests {
    #[test]
    fn markdown_export_basic_structure()
    #[test]
    fn html_export_with_styles()
    #[test]
    fn plain_text_strips_formatting()
    #[test]
    fn invalid_format_returns_error()
}
```

**2. 集成测试** (`tests/` 目录)

```
tests/
  ├── integration/
  │   ├── export_workflow_test.rs  // 完整导出流程
  │   ├── revision_workflow_test.rs // 版本审批流程
  │   └── search_filter_test.rs    // 搜索筛选
```

测试场景：
- 创建故事 → 审批 → 导出为各种格式
- 多次修订 → 版本对比 → 回滚
- 批量操作流程

**3. 测试辅助工具**

```rust
// tests/common/mod.rs
pub fn create_test_package() -> Value { ... }
pub fn create_test_workspace(temp_dir: &Path) -> RevisionManager { ... }
pub fn assert_valid_markdown(content: &str) { ... }
```

#### B. 前端组件测试 (2-3h)

**方案**: 使用 Vitest + @testing-library/svelte

**1. 配置测试环境**

安装依赖：
```json
{
  "devDependencies": {
    "vitest": "^2.0.0",
    "@testing-library/svelte": "^5.0.0",
    "@testing-library/jest-dom": "^6.0.0",
    "jsdom": "^25.0.0"
  }
}
```

Vite 配置：
```typescript
// vite.config.ts
import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts']
  }
})
```

**2. 组件测试**

```
src/lib/__tests__/
  ├── ArtifactBrowser.test.ts
  ├── RevisionWorkspace.test.ts
  ├── StoryJobForm.test.ts
  └── reading-mode.test.ts
```

测试示例：
```typescript
// ArtifactBrowser.test.ts
import { render, screen, fireEvent } from '@testing-library/svelte'
import ArtifactBrowser from '../ArtifactBrowser.svelte'

describe('ArtifactBrowser', () => {
  it('renders search input', () => {
    render(ArtifactBrowser)
    expect(screen.getByPlaceholderText(/搜索/)).toBeInTheDocument()
  })

  it('filters runs by search query', async () => {
    const { component } = render(ArtifactBrowser)
    const input = screen.getByPlaceholderText(/搜索/)
    await fireEvent.input(input, { target: { value: 'test' } })
    // Assert filtered results
  })

  it('toggles batch mode', async () => {
    render(ArtifactBrowser)
    const btn = screen.getByText(/批量操作/)
    await fireEvent.click(btn)
    expect(screen.getByText(/退出批量/)).toBeInTheDocument()
  })
})
```

**3. 阅读模式功能测试**

```typescript
// reading-mode.test.ts
describe('Reading Mode Enhancements', () => {
  it('toggles fullscreen mode', async () => { ... })
  it('adjusts font size within bounds', async () => { ... })
  it('jumps to episode on click', async () => { ... })
  it('toggles bookmark', async () => { ... })
  it('closes on ESC key', async () => { ... })
})
```

#### C. 测试覆盖率报告 (30min)

**配置 cargo-tarpaulin**（Rust）:
```yaml
# .gitlab-ci.yml
test:coverage:
  stage: test
  script:
    - cargo install cargo-tarpaulin
    - cargo tarpaulin --out Html --output-dir coverage
  artifacts:
    paths:
      - coverage/
```

**配置 Vitest 覆盖率**:
```json
{
  "scripts": {
    "test": "vitest",
    "test:coverage": "vitest run --coverage"
  },
  "devDependencies": {
    "@vitest/coverage-v8": "^2.0.0"
  }
}
```

---

## 🎯 目标 2: 主题切换

### 设计方案

**方案**: CSS 变量 + localStorage 持久化

### 实施步骤

#### 1. 定义 CSS 变量 (30min)

```css
/* app.css */
:root {
  /* 暗色主题（默认） */
  --bg-primary: #0d0f0c;
  --bg-secondary: #12140f;
  --bg-tertiary: #1a1d16;
  --fg-primary: #edeee8;
  --fg-secondary: #8d9484;
  --fg-tertiary: #62685c;
  --border-primary: #2b2f27;
  --border-secondary: #353a30;
  --accent-primary: #d7ff48;
  --accent-secondary: #8ba52f;
}

:root[data-theme="light"] {
  /* 亮色主题 */
  --bg-primary: #ffffff;
  --bg-secondary: #f5f5f5;
  --bg-tertiary: #e8e8e8;
  --fg-primary: #1a1a1a;
  --fg-secondary: #666666;
  --fg-tertiary: #999999;
  --border-primary: #d0d0d0;
  --border-secondary: #c0c0c0;
  --accent-primary: #6b8e23;
  --accent-secondary: #556b2f;
}

/* 应用变量到所有组件 */
body {
  background: var(--bg-primary);
  color: var(--fg-primary);
}

.sidebar {
  background: var(--bg-secondary);
  border-right: 1px solid var(--border-primary);
}

/* ... 更多样式 */
```

#### 2. 主题切换逻辑 (20min)

```typescript
// src/lib/theme.ts
export type Theme = 'light' | 'dark'

export function getTheme(): Theme {
  const stored = localStorage.getItem('theme')
  if (stored === 'light' || stored === 'dark') {
    return stored
  }
  // 检测系统偏好
  return window.matchMedia('(prefers-color-scheme: light)').matches 
    ? 'light' 
    : 'dark'
}

export function setTheme(theme: Theme) {
  localStorage.setItem('theme', theme)
  document.documentElement.setAttribute('data-theme', theme)
}

export function toggleTheme(): Theme {
  const current = getTheme()
  const next = current === 'dark' ? 'light' : 'dark'
  setTheme(next)
  return next
}

// 初始化主题
export function initTheme() {
  const theme = getTheme()
  document.documentElement.setAttribute('data-theme', theme)
}
```

#### 3. UI 集成 (20min)

**在 App.svelte 中初始化**:
```svelte
<script lang="ts">
  import { onMount } from 'svelte'
  import { initTheme } from './lib/theme'
  
  onMount(() => {
    initTheme()
  })
</script>
```

**添加切换按钮到侧边栏**:
```svelte
<!-- App.svelte 或 sidebar 区域 -->
<script lang="ts">
  import { toggleTheme, getTheme } from './lib/theme'
  let currentTheme = $state<'light' | 'dark'>(getTheme())
  
  function handleToggleTheme() {
    currentTheme = toggleTheme()
  }
</script>

<button class="theme-toggle" onclick={handleToggleTheme}>
  {currentTheme === 'dark' ? '☀️' : '🌙'}
  {currentTheme === 'dark' ? '亮色' : '暗色'}
</button>
```

**样式**:
```css
.theme-toggle {
  padding: 0.75rem 1rem;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-primary);
  border-radius: 8px;
  color: var(--fg-primary);
  display: flex;
  align-items: center;
  gap: 0.5rem;
  transition: all 0.2s ease;
}

.theme-toggle:hover {
  background: var(--bg-secondary);
  border-color: var(--accent-secondary);
}
```

#### 4. 阅读器主题适配 (10min)

确保阅读器使用 CSS 变量：
```css
.story-reader {
  background: var(--bg-primary);
  color: var(--fg-primary);
}

.story-reader-heading {
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-primary);
}

.episode-navigator {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-primary);
}
```

---

## 📂 文件清单

### 新增文件

**测试相关**:
```
apps/desktop/src/test-setup.ts
apps/desktop/src/lib/__tests__/
  ├── ArtifactBrowser.test.ts
  ├── RevisionWorkspace.test.ts
  ├── StoryJobForm.test.ts
  └── reading-mode.test.ts

crates/story-storage/src/export_formats.rs  (添加 #[cfg(test)])
tests/
  ├── common/
  │   └── mod.rs
  └── integration/
      ├── export_workflow_test.rs
      ├── revision_workflow_test.rs
      └── search_filter_test.rs

docs/TESTING_GUIDE.md
```

**主题相关**:
```
apps/desktop/src/lib/theme.ts
docs/THEME_SYSTEM.md
```

### 修改文件

```
apps/desktop/package.json          (添加测试依赖和脚本)
apps/desktop/vite.config.ts        (添加测试配置)
apps/desktop/src/app.css           (重构为 CSS 变量)
apps/desktop/src/App.svelte        (初始化主题)
.gitlab-ci.yml                     (添加覆盖率任务)
```

---

## ⚠️ 技术决策

### 测试框架选择

**Rust**: 标准 `#[test]` + cargo test
- ✅ 零配置
- ✅ 项目已使用
- ✅ CI 已集成

**前端**: Vitest + @testing-library/svelte
- ✅ 与 Vite 原生集成
- ✅ 快速（基于 Vite）
- ✅ Jest 兼容 API
- ❌ 需要配置（约 30min）

**不选择 Jest**: 配置复杂，与 Vite 集成不佳

### 主题系统架构

**CSS 变量方案** vs 多个 CSS 文件:
- ✅ 运行时切换无闪烁
- ✅ 易于维护
- ✅ 性能更好
- ✅ 支持平滑过渡动画

**localStorage 持久化** vs Tauri store:
- ✅ 更简单
- ✅ 前端独立
- ✅ 符合 Web 标准
- ❌ 跨设备不同步（可接受）

---

## 🚧 风险和限制

### 测试覆盖

**风险**:
1. Mock Tauri API 可能复杂
2. E2E 测试需要 Tauri 测试环境

**缓解**:
1. 先聚焦单元测试和组件测试
2. E2E 留给后续迭代

### 主题切换

**限制**:
1. 不包括自定义颜色
2. 只有两个预设主题
3. 某些第三方组件可能不适配

**验证**:
- 手动测试所有页面
- 确保对比度符合 WCAG AA

---

## ✅ 验收标准

### 测试覆盖

- [ ] Rust 后端核心模块单元测试覆盖率 > 70%
- [ ] 至少 3 个集成测试场景通过
- [ ] 前端 4 个主要组件有基础测试
- [ ] 阅读模式 5 个功能点有测试
- [ ] CI 自动运行所有测试
- [ ] 覆盖率报告可访问

### 主题切换

- [ ] 切换按钮在侧边栏可见
- [ ] 点击切换立即生效无闪烁
- [ ] 刷新页面保持上次选择
- [ ] 所有页面样式正确适配
- [ ] 阅读器模式正确适配
- [ ] 亮色主题对比度符合 WCAG AA

---

## 📝 实施顺序

### 第一阶段: 主题切换 (1-2h)
优先做主题，因为更快见效，可以先完成一个目标。

1. 定义 CSS 变量
2. 实现主题切换逻辑
3. 添加 UI 控件
4. 测试和调整

### 第二阶段: Rust 单元测试 (2-3h)
5. 为 export_formats.rs 添加测试
6. 为其他核心模块添加测试
7. 创建测试辅助工具

### 第三阶段: 前端测试 (2-3h)
8. 配置 Vitest
9. 编写组件测试
10. 编写阅读模式功能测试

### 第四阶段: 集成和报告 (1-2h)
11. 编写集成测试
12. 配置覆盖率报告
13. 更新 CI 配置
14. 编写文档

---

## 📚 参考资源

- Rust 测试: https://doc.rust-lang.org/book/ch11-00-testing.html
- Vitest: https://vitest.dev/
- Testing Library Svelte: https://testing-library.com/docs/svelte-testing-library/intro
- CSS 变量: https://developer.mozilla.org/en-US/docs/Web/CSS/Using_CSS_custom_properties
