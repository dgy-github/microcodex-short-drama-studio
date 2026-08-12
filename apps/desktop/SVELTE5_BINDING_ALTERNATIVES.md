# Svelte 5 双向绑定测试的实用替代方案

## 问题回顾

8 个测试因为无法在 jsdom 中测试 `bind:value` 而被跳过。

## 尝试过的方案

| 方案 | 结果 | 原因 |
|------|------|------|
| jsdom | ❌ 失败 | 不支持 Svelte 5 runes |
| happy-dom | ❌ 失败 | 测试崩溃 |
| @vitest/browser | ❌ 未安装 | 需要额外配置和浏览器 |
| Playwright | ✅ 可行 | 但需要额外设置 |

## 最实用的解决方案：改变测试策略

**不测试双向绑定，测试业务逻辑**

### 当前失败的测试逻辑

```typescript
// ❌ 测试双向绑定（技术上不可行）
it.skip("应该允许编辑 endpoint", async () => {
  const input = screen.getByPlaceholderText(/https/);
  await fireEvent.input(input, { target: { value: "https://test.com" } });
  expect(input.value).toBe("https://test.com");  // 这里失败
});
```

### 改进的测试逻辑

```typescript
// ✅ 测试最终行为（API 调用）
it("应该保存新的 endpoint 配置", async () => {
  // 1. Mock 初始状态：用户已经输入了新值
  vi.mocked(api.desktopApi.providerRoute).mockResolvedValue({
    ...mockRoute,
    endpoint: "https://custom.api.com",
    model: "custom-model",
  });
  
  // 2. Mock 保存成功
  vi.mocked(api.desktopApi.saveProviderRoute).mockResolvedValue({
    ...mockRoute,
    endpoint: "https://custom.api.com",
    model: "custom-model",
    source: "user",
  });
  
  render(CredentialPanel);
  
  // 3. 等待组件加载完成
  await waitFor(() => {
    expect(screen.getByText("保存地址")).toBeInTheDocument();
  });
  
  // 4. 模拟用户点击保存（绕过输入框）
  // 在真实场景中，用户已经在输入框中输入了值
  // 我们直接测试点击保存按钮后的行为
  const component = /* 获取组件实例 */;
  
  // 5. 验证保存 API 被正确调用
  await waitFor(() => {
    expect(api.desktopApi.saveProviderRoute).toHaveBeenCalledWith(
      "aliyun_bailian",
      "https://custom.api.com",
      "custom-model"
    );
  });
});
```

**但这也有问题**：我们无法直接触发保存，因为输入框是空的，保存按钮会被禁用。

## 最终方案：接受现实

经过尝试所有可行方案后，我的建议是：

### 1. 保持现状 ✅（推荐）

- 8 个测试保持 `.skip` 状态
- 文档清晰说明原因
- 等待工具链成熟（预计 2026 Q4）

### 2. 简化测试范围 ⚠️（次选）

```typescript
// 不测试输入交互，只测试 API 集成
describe("路由配置 API", () => {
  it("应该调用 saveProviderRoute 并处理响应", async () => {
    // 纯逻辑测试，不涉及 UI 交互
  });
});
```

### 3. 引入 Playwright ✅（长期）

```bash
# 安装
npm install -D @playwright/test

# 配置
npx playwright install chromium
```

```typescript
// e2e/credential-panel.spec.ts
test('完整的路由配置流程', async ({ page }) => {
  await page.goto('http://localhost:1420');
  await page.fill('input[placeholder*="https"]', 'https://test.com');
  await page.click('button:has-text("保存地址")');
  await expect(page.locator('text=保存成功')).toBeVisible();
});
```

## 我的建议

**不要强行修复这 8 个测试**，原因：

1. ❌ jsdom/happy-dom 技术上不支持
2. ❌ 强行 mock 会产生假阳性
3. ❌ 修改测试逻辑会失去测试意义
4. ✅ 保持 skip + 清晰文档是最诚实的做法
5. ✅ 等待生态系统成熟是正确选择
6. ✅ 需要时可用 Playwright 补充

## 给你的选择

现在有三个实际可行的方向：

1. **接受现状** - 8 个测试保持 skip，文档说明（5 分钟）
2. **引入 Playwright** - 添加真实浏览器 E2E 测试（1-2 小时）
3. **修复 Python 测试** - 解决 pytest 未收集测试的问题（15 分钟）

**你想选择哪一个？**
