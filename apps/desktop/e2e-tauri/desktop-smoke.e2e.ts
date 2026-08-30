describe("真实 Tauri WebView 桌面验收", () => {
  before(async () => {
    await browser.waitUntil(
      async () => (await browser.getTitle()) === "MicrocodeX 短剧工作室",
      { timeout: 30_000, timeoutMsg: "Tauri WebView 未完成启动" },
    );
  });

  beforeEach(async () => {
    await browser.waitUntil(
      async () => (await browser.getTitle()) === "MicrocodeX 短剧工作室",
      { timeout: 30_000, timeoutMsg: "Tauri WebView 未完成初始化" },
    );
  });

  it("启动真实桌面 WebView 并通过 IPC 初始化创作台", async () => {
    await expect(await $("nav[aria-label='主导航']")).toBeDisplayed();
    // On a machine without configured provider credentials the app opens on
    // 模型配置 by design, so the 创作台 view has to be requested explicitly.
    await (await $("button=创作台")).click();
    await expect(await $("h1=创作台")).toBeDisplayed();
    await expect(await $("h2=把一句想法变成制作任务")).toBeDisplayed();
    await browser.waitUntil(
      async () => !(await browser.getPageSource()).includes("CONNECTING"),
      { timeout: 15_000, timeoutMsg: "桌面 IPC 状态未完成初始化" },
    );
  });

  it("通过真实 Tauri IPC 加载并编辑模型路由", async () => {
    await (await $("button=模型配置")).click();
    await expect(await $("h2=本机凭据保险箱")).toBeDisplayed();

    const endpointInputs = await $$("input[type='url']");
    const modelInputs = await $$("input[placeholder='模型 ID']");
    expect(endpointInputs).toHaveLength(2);
    expect(modelInputs).toHaveLength(2);

    await endpointInputs[1].setValue("https://test.invalid/v1/chat/completions");
    await modelInputs[1].setValue("e2e-model");
    await expect(endpointInputs[1]).toHaveValue("https://test.invalid/v1/chat/completions");
    await expect(modelInputs[1]).toHaveValue("e2e-model");
    await expect((await $$("button=保存地址"))[1]).toBeEnabled();
  });

  it("通过真实 Tauri IPC 加载评测目录并打开案例详情", async () => {
    await (await $("button=评测中心")).click();
    await expect(await $("h2=评测中心")).toBeDisplayed();
    const firstCase = await $("[aria-label^='评测用例 ']");
    await expect(firstCase).toBeDisplayed();
    await firstCase.doubleClick();
    await expect(await $("button[aria-label='关闭用例详情']")).toBeDisplayed();
    await browser.keys(["Escape"]);
    await expect(await $("button[aria-label='关闭用例详情']")).not.toBeDisplayed();
  });

  it("通过真实 Tauri IPC 加载作品库并支持搜索", async () => {
    await (await $("button=作品库")).click();
    await expect(await $("h2=已完成的故事包")).toBeDisplayed();
    const search = await $("input[placeholder='搜索标题、运行ID、模型...']");
    await expect(search).toBeDisplayed();
    await search.setValue("不存在的作品");
    await expect(search).toHaveValue("不存在的作品");
    const source = await browser.getPageSource();
    expect(source).not.toContain("操作失败，请检查本地配置后重试。");
  });
});
