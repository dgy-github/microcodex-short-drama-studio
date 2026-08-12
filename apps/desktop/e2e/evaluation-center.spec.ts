import { test, expect } from '@playwright/test';

test.describe('EvaluationCenter - 人工评估', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // 等待评估中心加载
    await page.waitForSelector('text=自动评测', { timeout: 10000 });
  });

  test('应该切换到人工评估模式', async ({ page }) => {
    // 点击人工盲测按钮
    const humanModeButton = page.locator('button:has-text("人工盲测")');
    await humanModeButton.click();

    // 验证切换成功：应该显示"创建所选盲测"按钮
    await expect(page.locator('button:has-text("创建所选盲测")')).toBeVisible();
  });

  test('应该创建盲评任务', async ({ page }) => {
    // 1. 切换到人工盲测模式
    await page.locator('button:has-text("人工盲测")').click();

    // 2. 等待用例列表加载
    await page.waitForSelector('input[type="checkbox"]', { timeout: 10000 });

    // 3. 选择第一个用例
    const firstCheckbox = page.locator('input[type="checkbox"]').first();
    await firstCheckbox.check();

    // 4. 点击创建盲测按钮
    await page.locator('button:has-text("创建所选盲测")').click();

    // 5. 验证盲评任务创建成功（应该显示评分界面）
    await expect(page.locator('text=连贯性')).toBeVisible({ timeout: 10000 });
  });

  test('应该显示评分界面', async ({ page }) => {
    // 1. 切换到人工盲测并创建任务
    await page.locator('button:has-text("人工盲测")').click();

    const firstCheckbox = page.locator('input[type="checkbox"]').first();
    await firstCheckbox.check();

    await page.locator('button:has-text("创建所选盲测")').click();

    // 2. 验证评分界面元素
    await expect(page.locator('text=连贯性')).toBeVisible();
    await expect(page.locator('text=情节逻辑是否连贯')).toBeVisible();

    // 3. 验证有评分选择器
    const scoreSelects = page.locator('select');
    await expect(scoreSelects.first()).toBeVisible();
  });

  test('应该允许输入评分', async ({ page }) => {
    // 1. 进入评分界面
    await page.locator('button:has-text("人工盲测")').click();
    const firstCheckbox = page.locator('input[type="checkbox"]').first();
    await firstCheckbox.check();
    await page.locator('button:has-text("创建所选盲测")').click();

    // 2. 等待评分界面
    await page.waitForSelector('select', { timeout: 10000 });

    // 3. 选择评分
    const scoreSelect = page.locator('select').first();
    await scoreSelect.selectOption('5');

    // 4. 验证选择成功
    await expect(scoreSelect).toHaveValue('5');
  });
});

test.describe('EvaluationCenter - 案例详情', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('text=自动评测', { timeout: 10000 });
  });

  test('应该打开案例详情', async ({ page }) => {
    // 等待用例列表加载
    await page.waitForSelector('text=测试故事1', { timeout: 10000 });

    // 找到第一个用例行
    const caseRow = page.locator('tr:has-text("测试故事1")');

    // 双击打开详情
    await caseRow.dblclick();

    // 验证详情面板打开（应该显示 case_id）
    await expect(page.locator('text=case_001')).toBeVisible({ timeout: 5000 });
  });

  test('应该通过 Escape 键关闭详情', async ({ page }) => {
    // 1. 打开详情
    await page.waitForSelector('text=测试故事1', { timeout: 10000 });
    const caseRow = page.locator('tr:has-text("测试故事1")');
    await caseRow.dblclick();

    // 2. 验证详情已打开
    await expect(page.locator('text=case_001')).toBeVisible();

    // 3. 按 Escape 键
    await page.keyboard.press('Escape');

    // 4. 验证详情已关闭
    await expect(page.locator('text=case_001')).not.toBeVisible({ timeout: 5000 });
  });
});

test.describe('EvaluationCenter - 错误处理', () => {
  test.skip('应该处理目录加载失败', async ({ page }) => {
    // 这个测试需要 mock API 失败
    // 在 E2E 测试中较难实现，保持 skip
    // 或者使用 page.route() 拦截请求
  });
});
