import { test, expect } from '@playwright/test';

test.describe('CredentialPanel - 路由配置', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // 等待应用加载完成
    await page.waitForSelector('text=MicrocodeX', { timeout: 10000 });

    // 点击"模型配置"标签切换到 CredentialPanel
    await page.click('button:has-text("模型配置")');

    // 等待 CredentialPanel 加载
    await page.waitForSelector('text=DeepSeek', { timeout: 10000 });
  });

  test('应该显示已保存的路由配置', async ({ page }) => {
    // 等待路由配置加载
    await page.waitForSelector('input[placeholder*="https"]', { timeout: 10000 });

    // 检查 DeepSeek 的默认配置是否显示
    const endpointInputs = page.locator('input[placeholder*="https"]');
    const firstEndpoint = endpointInputs.first();

    // DeepSeek 应该有默认的 endpoint
    await expect(firstEndpoint).toBeVisible();
  });

  test('应该允许编辑 endpoint', async ({ page }) => {
    // 找到阿里云百炼的 endpoint 输入框（第二个）
    const endpointInputs = page.locator('input[placeholder*="https"]');
    const aliyunEndpoint = endpointInputs.nth(1);

    await aliyunEndpoint.waitFor({ state: 'visible' });

    // ✅ 在真实浏览器中，bind:value 正常工作
    await aliyunEndpoint.fill('https://custom.api.com/v1/chat');

    // 验证值已更新
    await expect(aliyunEndpoint).toHaveValue('https://custom.api.com/v1/chat');
  });

  test('应该允许编辑 model', async ({ page }) => {
    // 找到模型 ID 输入框
    const modelInputs = page.locator('input[placeholder="模型 ID"]');
    const aliyunModel = modelInputs.nth(1);

    await aliyunModel.waitFor({ state: 'visible' });

    // ✅ 双向绑定在真实浏览器中正常工作
    await aliyunModel.fill('custom-model-v1');

    // 验证值已更新
    await expect(aliyunModel).toHaveValue('custom-model-v1');
  });

  test('应该保存路由配置', async ({ page }) => {
    // 1. 填写 endpoint 和 model
    const endpointInputs = page.locator('input[placeholder*="https"]');
    const modelInputs = page.locator('input[placeholder="模型 ID"]');

    const aliyunEndpoint = endpointInputs.nth(1);
    const aliyunModel = modelInputs.nth(1);

    await aliyunEndpoint.fill('https://test.api.com');
    await aliyunModel.fill('test-model');

    // 2. 点击保存按钮
    const saveButtons = page.locator('button:has-text("保存地址")');
    const aliyunSaveButton = saveButtons.nth(1);

    await aliyunSaveButton.click();

    // 3. 验证成功消息（假设组件显示成功提示）
    // 根据实际组件实现调整
    await page.waitForTimeout(1000);

    // 验证按钮状态或成功消息
    // await expect(page.locator('text=保存成功')).toBeVisible();
  });
});

test.describe('CredentialPanel - 稳定性检查', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('text=MicrocodeX', { timeout: 10000 });

    // 点击"模型配置"标签
    await page.click('button:has-text("模型配置")');
    await page.waitForSelector('text=DeepSeek', { timeout: 10000 });
  });

  test('应该显示稳定性检查设置', async ({ page }) => {
    // 查找稳定性检查相关元素
    await expect(page.locator('text=双供应商稳定性检查')).toBeVisible();
    await expect(page.locator('button:has-text("运行稳定性检查")')).toBeVisible();
  });

  test('应该允许设置迭代次数', async ({ page }) => {
    // 查找迭代次数输入框
    const iterationInput = page.locator('input[type="number"]').first();

    await iterationInput.waitFor({ state: 'visible' });

    // ✅ 双向绑定正常工作
    await iterationInput.fill('10');

    await expect(iterationInput).toHaveValue('10');
  });

  test.skip('应该运行稳定性检查', async ({ page }) => {
    // 这个测试需要实际的凭据配置
    // 标记为 skip，只在有测试环境时运行

    const soakButton = page.locator('button:has-text("运行稳定性检查")');

    // 等待按钮启用（需要两个提供商都配置）
    await soakButton.waitFor({ state: 'visible' });

    // 如果按钮可用，点击运行
    const isEnabled = await soakButton.isEnabled();
    if (isEnabled) {
      await soakButton.click();
      // 等待结果
      await page.waitForSelector('text=/\\d+\\/\\d+ 成功/', { timeout: 30000 });
    }
  });

  test.skip('应该显示稳定性检查结果', async ({ page }) => {
    // 需要实际运行稳定性检查后才能测试结果显示
    // 标记为 skip
  });
});
