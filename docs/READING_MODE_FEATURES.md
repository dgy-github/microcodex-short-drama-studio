# 📖 阅读模式增强功能

## 概述

为故事阅读器添加了四项用户体验提升功能，让用户能更舒适地阅读和浏览长篇故事。

---

## ✨ 新增功能

### 1. 全屏阅读模式 🖥️

**功能说明**：
- 点击工具栏的全屏按钮（⊡）进入全屏模式
- 再次点击（⊟）或按 ESC 键退出全屏
- 全屏模式下，阅读器占据整个屏幕，提供沉浸式阅读体验

**适用场景**：
- 长时间阅读
- 需要专注时减少干扰
- 演示或展示故事内容

**技术实现**：
```typescript
let fullscreenMode = $state(false);

function toggleFullscreen() {
  fullscreenMode = !fullscreenMode;
}
```

---

### 2. 字体大小调节 🔤

**功能说明**：
- **A-** 按钮：减小字体（最小 12px）
- **A** 按钮：重置为默认大小（16px）
- **A+** 按钮：增大字体（最大 24px）
- 每次调节增减 2px

**适用场景**：
- 视力调节需求
- 不同屏幕尺寸适配
- 个人阅读偏好

**技术实现**：
```typescript
let fontSize = $state(16);

function increaseFontSize() {
  if (fontSize < 24) fontSize += 2;
}

function decreaseFontSize() {
  if (fontSize > 12) fontSize -= 2;
}
```

CSS 应用：
```svelte
<div style="font-size: {fontSize}px;">
  <!-- 内容 -->
</div>
```

---

### 3. 快速跳转到某集 🎯

**功能说明**：
- 分集正文上方显示导航栏
- 显示所有集数的快捷按钮（1, 2, 3...）
- 点击按钮平滑滚动到对应集数
- 当前查看集数高亮显示
- 带书签的集数有特殊标识（★）

**适用场景**：
- 长篇故事快速定位
- 跳过已读内容
- 重新查看特定集数

**技术实现**：
```typescript
let currentEpisodeIndex = $state(0);

function jumpToEpisode(index: number) {
  currentEpisodeIndex = index;
  const episodeElement = document.getElementById(`episode-${index}`);
  if (episodeElement) {
    episodeElement.scrollIntoView({ 
      behavior: "smooth", 
      block: "start" 
    });
  }
}
```

HTML 标识：
```svelte
<article class="story-episode" id="episode-{index}">
  <!-- 集数内容 -->
</article>
```

导航栏：
```svelte
<div class="episode-navigator">
  <span class="nav-label">快速跳转：</span>
  <div class="episode-jump-buttons">
    {#each episodes as episode, index}
      <button
        class="episode-jump-btn"
        class:active={currentEpisodeIndex === index}
        onclick={() => jumpToEpisode(index)}
      >
        {index + 1}
      </button>
    {/each}
  </div>
</div>
```

---

### 4. 书签功能 🔖

**功能说明**：
- 每集标题右侧显示书签按钮（☆/★）
- 点击可添加或移除书签
- 已加书签的集数在导航栏中特殊显示
- 书签状态在当前会话中保持

**适用场景**：
- 标记重要或喜欢的集数
- 稍后返回特定内容
- 标记需要重读的部分

**技术实现**：
```typescript
let bookmarks = $state<Set<string>>(new Set());

function toggleBookmark(nodeId: string) {
  if (bookmarks.has(nodeId)) {
    bookmarks.delete(nodeId);
  } else {
    bookmarks.add(nodeId);
  }
  bookmarks = new Set(bookmarks);
}

function isBookmarked(nodeId?: string) {
  return nodeId ? bookmarks.has(nodeId) : false;
}
```

UI 展示：
```svelte
<button
  class="ghost bookmark-btn"
  onclick={() => toggleBookmark(episode.node_id)}
>
  {isBookmarked(episode.node_id) ? "★" : "☆"}
</button>
```

---

## 🎨 UI/UX 设计

### 颜色方案

- **主色**：`#d7ff48`（荧光绿）- 高亮和活动状态
- **次级色**：`#8ba52f`（橄榄绿）- 悬停和选中状态
- **背景**：`#0d0f0c` / `#12140f`（深灰）- 主背景
- **前景文字**：`#edeee8` / `#eff0ea`（浅灰）
- **边框**：`#2b2f27` / `#353a30`（中灰）

### 交互反馈

所有按钮都有：
- **悬停态**：背景变亮，边框高亮
- **活动态**：使用主色背景
- **禁用态**：透明度 45%
- **过渡动画**：0.2s ease

### 响应式布局

- 导航栏使用 `sticky` 定位，滚动时保持可见
- 全屏模式下标题栏也 sticky，始终可见控制按钮
- 快速跳转按钮自动换行，适应不同屏幕宽度

---

## 🎹 键盘快捷键

| 快捷键 | 功能 |
|--------|------|
| `ESC` | 退出阅读器或全屏模式 |

**提示**：键盘快捷键提示显示在工具栏左侧

---

## 📱 可访问性（Accessibility）

### ARIA 属性

- 阅读器使用 `role="dialog"` 和 `aria-modal="true"`
- 所有按钮都有 `aria-label` 描述功能
- 标题使用 `aria-labelledby` 关联

### 键盘导航

- 所有交互元素支持键盘访问
- Tab 键按逻辑顺序导航
- ESC 键退出模态对话框

### 视觉对比

- 文字与背景对比度符合 WCAG AA 标准
- 活动状态使用多种视觉线索（颜色、边框、阴影）
- 不依赖单一颜色传达信息

---

## 🔧 技术细节

### 状态管理

使用 Svelte 5 的 `$state` rune：
```typescript
let fullscreenMode = $state(false);
let fontSize = $state(16);
let currentEpisodeIndex = $state(0);
let bookmarks = $state<Set<string>>(new Set());
```

### CSS 类切换

```svelte
<div 
  class="story-reader"
  class:fullscreen={fullscreenMode}
  style="font-size: {fontSize}px;"
>
```

### 平滑滚动

```css
.story-reader {
  scroll-behavior: smooth;
}
```

### Sticky 定位

```css
.episode-navigator {
  position: sticky;
  top: 0;
  z-index: 10;
  backdrop-filter: blur(10px);
}
```

---

## 📦 涉及文件

### 前端组件
- `apps/desktop/src/lib/ArtifactBrowser.svelte`
  - 添加状态变量（第 26-29 行）
  - 添加控制函数（第 99-137 行）
  - 更新 UI 结构（第 354-472 行）

### 样式文件
- `apps/desktop/src/app.css`
  - 添加阅读模式样式（约 200 行新增）

---

## 🚀 使用方法

1. **启动应用**
   ```bash
   cd apps/desktop
   cargo tauri dev
   ```

2. **打开故事阅读器**
   - 进入「作品库」
   - 双击任意故事或点击「查看完整故事」

3. **使用新功能**
   - 点击工具栏按钮调节字体大小
   - 点击全屏按钮进入沉浸式阅读
   - 使用顶部导航栏快速跳转
   - 点击集数标题旁的星标添加书签

---

## 🐛 已知限制

1. **书签持久化**：书签仅在当前会话保持，关闭阅读器后清空
2. **跨平台全屏**：全屏模式是应用内全屏，不是系统级全屏
3. **字体范围**：字体大小限制在 12-24px，不支持更大或更小

---

## 🔮 未来改进方向

### 短期（1-2 小时）
- [ ] 书签持久化到本地存储
- [ ] 记住上次阅读位置
- [ ] 添加阅读进度条

### 中期（3-5 小时）
- [ ] 夜间模式/亮色模式切换
- [ ] 更多字体选项（字体家族、行距等）
- [ ] 分集缩略图预览
- [ ] 键盘快捷键扩展（翻页、跳转等）

### 长期（5+ 小时）
- [ ] 阅读统计（阅读时间、进度等）
- [ ] 笔记和高亮功能
- [ ] 语音朗读
- [ ] 跨设备同步（书签、进度等）

---

## 📊 测试建议

### 功能测试
- [ ] 全屏模式开关正常
- [ ] 字体调节范围正确
- [ ] 快速跳转定位准确
- [ ] 书签添加/移除正常

### UI 测试
- [ ] 各按钮悬停效果正常
- [ ] 活动状态高亮正确
- [ ] 导航栏 sticky 定位有效
- [ ] 响应式布局适配良好

### 兼容性测试
- [ ] Windows 10/11
- [ ] 不同分辨率屏幕
- [ ] 不同 DPI 缩放

### 性能测试
- [ ] 长故事（50+ 集）跳转流畅
- [ ] 频繁切换全屏无卡顿
- [ ] 大量书签操作响应快速

---

## 📝 更新日志

### 2026-08-12
- ✅ 实现全屏阅读模式
- ✅ 实现字体大小调节
- ✅ 实现快速跳转功能
- ✅ 实现书签功能
- ✅ 添加 CSS 样式和动画
- ✅ 完善键盘快捷键支持

---

## 👥 贡献者

- 阅读模式增强功能设计与实现

---

**状态**: ✅ 已完成并待测试
**优先级**: 🔴 高
**预计测试时间**: 30 分钟
