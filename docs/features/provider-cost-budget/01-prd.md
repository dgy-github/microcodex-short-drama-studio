# REQ-421..423 — Provider cost budget

Status: G4 implementation complete; real pricing catalog required for live verification

## Requirements

- **REQ-421:** Rust 根据版本化 provider+model 精确费率和输入/输出 token 计算人民币分；
  未知 route、重复 route、无目录或溢出必须 fail-closed。
- **REQ-422:** 每个付费 task 的 `usage` 携带 `cost_cny_fen` 与
  `pricing_catalog_id`；sidecar 在 retain artifact 前同时执行 token 与费用上限。
- **REQ-423:** 进程恢复继续累计同一 run 的费用，desktop 按 durable sequence 去重投影。

## Acceptance

- 不在代码中猜测或硬编码供应商实时价格；
- 不存在 `config/provider-pricing-v1.json` 时生产故事 run 不启动；
- pricing catalog 必须与当前 provider settings 的 model 精确匹配；
- 超过 `max_cny_fen` 的 task 不进入 artifact retention；
- 恢复、重复事件和桌面显示使用同一 `cost_cny_fen` 口径。

示例文件中的 0 费率只是结构模板，不能直接复制为生产价格目录。操作员必须从供应商
当前账单规则核验输入/输出每百万 token 的人民币分价格，填写真实 model，并保存为
`config/provider-pricing-v1.json`。付费 E2E 使用 `STORY_PRICING_CATALOG` 指向同类文件。
