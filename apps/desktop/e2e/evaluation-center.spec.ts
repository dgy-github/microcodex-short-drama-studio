import { test, expect } from '@playwright/test';

test.describe('EvaluationCenter - 人工评估', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // 等待应用加载
    await page.waitForSelector('text=MicrocodeX', { timeout: 10000 });

    // 点击"评测中心"标签切换到 EvaluationCenter
    await page.click('button:has-text("评测中心")');

    // 等待评估中心加载完成（等待按钮组出现）
    await page.waitForSelector('button:has-text("自动评测")', { timeout: 15000 });
  });

  test.skip('应该切换到人工评估模式 (需要评估数据)', async ({ page }) => {
    // 点击人工盲测按钮
    const humanModeButton = page.locator('button:has-text("人工盲测")');
    await humanModeButton.click();

    // 验证切换成功：按钮应该有 active 状态
    await expect(humanModeButton).toHaveClass(/active/);
  });

  test.skip('应该创建盲评任务 (需要评估数据)', async ({ page }) => {
    // 1. 切换到人工盲测模式
    await page.locator('button:has-text("人工盲测")').click();

    // 2. 等待用例列表加载
    await page.waitForSelector('input[type="checkbox"]', { timeout: 10000 });

    // 3. 选择第一个用例
    const firstCheckbox = page.locator('input[type="checkbox"]').first();
    await firstCheckbox.check();

    // 4. 点击创建盲测按钮
    const createButton = page.locator('button:has-text("创建所选盲测")');
    await createButton.click();

    // 5. 验证盲评任务创建成功（应该进入盲测界面）
    await expect(page.locator('button:has-text("退出盲测")')).toBeVisible({ timeout: 10000 });
  });

  test.skip('应该显示评分界面 (需要评估数据)', async ({ page }) => {
    // 1. 切换到人工盲测并创建任务
    await page.locator('button:has-text("人工盲测")').click();

    const firstCheckbox = page.locator('input[type="checkbox"]').first();
    await firstCheckbox.check();

    await page.locator('button:has-text("创建所选盲测")').click();

    // 2. 等待盲测界面加载
    await expect(page.locator('button:has-text("退出盲测")')).toBeVisible({ timeout: 10000 });

    // 3. 验证评分界面元素（根据实际组件结构调整）
    // 等待评分相关内容加载
    await page.waitForTimeout(1000);
  });

  test.skip('应该允许输入评分 (需要评估数据)', async ({ page }) => {
    // 1. 进入评分界面
    await page.locator('button:has-text("人工盲测")').click();
    const firstCheckbox = page.locator('input[type="checkbox"]').first();
    await firstCheckbox.check();
    await page.locator('button:has-text("创建所选盲测")').click();

    // 2. 等待盲测界面
    await expect(page.locator('button:has-text("退出盲测")')).toBeVisible({ timeout: 10000 });

    // 3. 等待评分界面加载
    await page.waitForTimeout(1000);

    // 4. 查找评分输入控件（select 或其他输入）
    const scoreInputs = page.locator('select, input[type="number"]');
    const firstInput = scoreInputs.first();

    if (await firstInput.isVisible()) {
      // 如果是 select，选择一个值
      if ((await firstInput.getAttribute('type')) !== 'number') {
        await firstInput.selectOption({ index: 1 });
      } else {
        // 如果是 number input，填入数字
        await firstInput.fill('5');
      }
    }
  });
});

test.describe('EvaluationCenter - 案例详情', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('text=MicrocodeX', { timeout: 10000 });

    // 点击"评测中心"标签
    await page.click('button:has-text("评测中心")');
    await page.waitForSelector('button:has-text("自动评测")', { timeout: 15000 });
  });

  test.skip('应该打开案例详情 (需要评估数据)', async ({ page }) => {
    // 等待用例列表加载
    await page.waitForSelector('input[type="checkbox"]', { timeout: 10000 });

    // 找到第一个用例行（包含复选框的行）
    const firstCheckbox = page.locator('input[type="checkbox"]').first();
    const caseRow = page.locator('tr').filter({ has: firstCheckbox });

    // 双击打开详情
    await caseRow.dblclick();

    // 验证详情面板打开（应该显示关闭按钮）
    await expect(page.locator('button[aria-label="关闭用例详情"]')).toBeVisible({ timeout: 5000 });
  });

  test.skip('应该通过 Escape 键关闭详情 (需要评估数据)', async ({ page }) => {
    // 1. 打开详情
    await page.waitForSelector('input[type="checkbox"]', { timeout: 10000 });
    const firstCheckbox = page.locator('input[type="checkbox"]').first();
    const caseRow = page.locator('tr').filter({ has: firstCheckbox });
    await caseRow.dblclick();

    // 2. 验证详情已打开
    await expect(page.locator('button[aria-label="关闭用例详情"]')).toBeVisible();

    // 3. 按 Escape 键
    await page.keyboard.press('Escape');

    // 4. 验证详情已关闭
    await expect(page.locator('button[aria-label="关闭用例详情"]')).not.toBeVisible({ timeout: 5000 });
  });
});

test.describe('EvaluationCenter - 错误处理', () => {
  test.skip('应该处理目录加载失败', async ({ page }) => {
    // 这个测试需要 mock API 失败
    // 在 E2E 测试中较难实现，保持 skip
    // 或者使用 page.route() 拦截请求
  });
});
