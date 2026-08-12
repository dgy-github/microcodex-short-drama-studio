import { test, expect } from '@playwright/test';

test.describe('ArtifactBrowser - 阅读器和修订工作区', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // 等待应用加载
    await page.waitForSelector('text=MicrocodeX', { timeout: 10000 });

    // 点击"作品库"标签切换到 ArtifactBrowser
    await page.click('button:has-text("作品库")');

    // 等待作品库加载
    await page.waitForSelector('h1:has-text("作品库")', { timeout: 10000 });
  });

  test('应该切换到阅读器视图', async ({ page }) => {
    // 查找阅读器视图切换按钮
    const readerButton = page.locator('button:has-text("阅读器")');

    if (await readerButton.isVisible()) {
      await readerButton.click();

      // 验证切换成功
      await expect(page.locator('.reader-view')).toBeVisible({ timeout: 5000 });
    }
  });

  test('应该在阅读器中显示内容', async ({ page }) => {
    // 1. 选择一个工件
    const firstArtifact = page.locator('.artifact-item').first();

    if (await firstArtifact.isVisible()) {
      await firstArtifact.click();

      // 2. 切换到阅读器
      const readerButton = page.locator('button:has-text("阅读器")');
      if (await readerButton.isVisible()) {
        await readerButton.click();

        // 3. 验证内容显示
        await expect(page.locator('.reader-content')).toBeVisible({ timeout: 5000 });
      }
    }
  });

  test('应该打开修订工作区', async ({ page }) => {
    // 查找修订工作区按钮
    const revisionButton = page.locator('button:has-text("修订工作区")');

    if (await revisionButton.isVisible()) {
      await revisionButton.click();

      // 验证修订工作区打开
      await expect(page.locator('text=修订历史')).toBeVisible({ timeout: 5000 });
    }
  });
});

test.describe('StoryJobForm - 类型包切换', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // 等待应用加载
    await page.waitForSelector('text=MicrocodeX', { timeout: 10000 });

    // 默认就在"创作台"，但确保已加载
    await page.waitForSelector('h1:has-text("创作台")', { timeout: 10000 });
  });

  test('应该允许切换到类型包', async ({ page }) => {
    // 查找类型包选项
    const typePackageRadio = page.locator('input[type="radio"][value="type-package"]');

    if (await typePackageRadio.isVisible()) {
      await typePackageRadio.check();

      // 验证切换成功（应该显示类型包相关字段）
      await expect(page.locator('.type-package-fields')).toBeVisible({ timeout: 5000 });
    }
  });

  test('应该切换回预设类型', async ({ page }) => {
    // 1. 先切换到类型包
    const typePackageRadio = page.locator('input[type="radio"][value="type-package"]');

    if (await typePackageRadio.isVisible()) {
      await typePackageRadio.check();
      await page.waitForTimeout(500);

      // 2. 切换回预设类型
      const presetRadio = page.locator('input[type="radio"][value="preset"]');
      await presetRadio.check();

      // 3. 验证切换成功（预设类型字段应该可见）
      await expect(page.locator('.preset-fields')).toBeVisible({ timeout: 5000 });
    }
  });
});

test.describe('RevisionWorkspace - 完整测试', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // 等待应用加载
    await page.waitForSelector('text=MicrocodeX', { timeout: 10000 });

    // 点击"作品库"标签
    await page.click('button:has-text("作品库")');
    await page.waitForSelector('h1:has-text("作品库")', { timeout: 10000 });
  });

  test('应该加载修订工作区', async ({ page }) => {
    // 导航到修订工作区（根据实际导航方式调整）
    const revisionTab = page.locator('button:has-text("修订工作区")');

    if (await revisionTab.isVisible()) {
      await revisionTab.click();

      // 验证工作区加载
      await expect(page.locator('text=修订工作区')).toBeVisible({ timeout: 10000 });
    }
  });

  test('应该显示故事标题', async ({ page }) => {
    // 导航到修订工作区
    const revisionTab = page.locator('button:has-text("修订工作区")');

    if (await revisionTab.isVisible()) {
      await revisionTab.click();

      // 验证故事标题显示
      const storyTitle = page.locator('.story-title');
      await expect(storyTitle).toBeVisible({ timeout: 10000 });
    }
  });

  test('应该显示修订列表', async ({ page }) => {
    // 导航到修订工作区
    const revisionTab = page.locator('button:has-text("修订工作区")');

    if (await revisionTab.isVisible()) {
      await revisionTab.click();

      // 验证修订列表显示
      const revisionList = page.locator('.revision-list');
      await expect(revisionList).toBeVisible({ timeout: 10000 });
    }
  });

  test('应该处理加载失败', async ({ page }) => {
    // 拦截 API 请求并返回错误
    await page.route('**/api/revisions/**', route => {
      route.abort('failed');
    });

    // 导航到修订工作区
    const revisionTab = page.locator('button:has-text("修订工作区")');

    if (await revisionTab.isVisible()) {
      await revisionTab.click();

      // 验证错误消息显示
      await expect(page.locator('text=/加载失败|错误/')).toBeVisible({ timeout: 10000 });
    }
  });

  test('应该选择缺陷', async ({ page }) => {
    // 导航到修订工作区并进入创建模式
    const revisionTab = page.locator('button:has-text("修订工作区")');

    if (await revisionTab.isVisible()) {
      await revisionTab.click();

      // 点击创建修订按钮
      const createButton = page.locator('button:has-text("创建修订")');
      if (await createButton.isVisible()) {
        await createButton.click();

        // 选择缺陷类型
        const defectCheckbox = page.locator('input[type="checkbox"]').first();
        await defectCheckbox.check();

        // 验证选择成功
        await expect(defectCheckbox).toBeChecked();
      }
    }
  });

  test('应该创建新修订', async ({ page }) => {
    // 导航到修订工作区
    const revisionTab = page.locator('button:has-text("修订工作区")');

    if (await revisionTab.isVisible()) {
      await revisionTab.click();

      // 点击创建修订
      const createButton = page.locator('button:has-text("创建修订")');
      if (await createButton.isVisible()) {
        await createButton.click();

        // 填写修订信息
        const defectCheckbox = page.locator('input[type="checkbox"]').first();
        await defectCheckbox.check();

        const commentInput = page.locator('textarea[placeholder*="修订说明"]');
        if (await commentInput.isVisible()) {
          await commentInput.fill('测试修订');
        }

        // 提交创建
        const submitButton = page.locator('button:has-text("提交")');
        if (await submitButton.isVisible()) {
          await submitButton.click();

          // 验证创建成功
          await expect(page.locator('text=/创建成功|修订已创建/')).toBeVisible({ timeout: 10000 });
        }
      }
    }
  });

  test('应该批准修订', async ({ page }) => {
    // 导航到修订工作区
    const revisionTab = page.locator('button:has-text("修订工作区")');

    if (await revisionTab.isVisible()) {
      await revisionTab.click();

      // 选择一个待审批的修订
      const pendingRevision = page.locator('.revision-item.pending').first();

      if (await pendingRevision.isVisible()) {
        await pendingRevision.click();

        // 点击批准按钮
        const approveButton = page.locator('button:has-text("批准")');
        await approveButton.click();

        // 验证批准成功
        await expect(page.locator('text=/已批准|批准成功/')).toBeVisible({ timeout: 10000 });
      }
    }
  });

  test('应该拒绝修订', async ({ page }) => {
    // 导航到修订工作区
    const revisionTab = page.locator('button:has-text("修订工作区")');

    if (await revisionTab.isVisible()) {
      await revisionTab.click();

      // 选择一个待审批的修订
      const pendingRevision = page.locator('.revision-item.pending').first();

      if (await pendingRevision.isVisible()) {
        await pendingRevision.click();

        // 点击拒绝按钮
        const rejectButton = page.locator('button:has-text("拒绝")');
        await rejectButton.click();

        // 填写拒绝原因
        const reasonInput = page.locator('textarea[placeholder*="拒绝原因"]');
        if (await reasonInput.isVisible()) {
          await reasonInput.fill('测试拒绝原因');
        }

        // 确认拒绝
        const confirmButton = page.locator('button:has-text("确认拒绝")');
        if (await confirmButton.isVisible()) {
          await confirmButton.click();

          // 验证拒绝成功
          await expect(page.locator('text=/已拒绝|拒绝成功/')).toBeVisible({ timeout: 10000 });
        }
      }
    }
  });
});
