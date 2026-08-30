# Provider cost budget design

`story-provider::PricingCatalog` 是唯一价格与计价所有者。它按 provider+model 精确匹配，
使用 u128 计算输入与输出 token 成本，向上取整到最小人民币分，避免低额调用被静默算成
0。Capability host 在 provider 成功返回 usage 后报价，并把 catalog identity 一起传给
Python。

Python workflow 只累计 Rust 提供的已报价费用，缺失费用视为 `provider_cost_unknown`；
超限视为 `cost_budget_exceeded`，发生在 artifact retention 之前。恢复从同一 run 的
durable `task.completed` 重建 token 与费用。desktop 同样从去重后的 completed event
投影费用，不重复计费。

供应商重试中未返回 usage 的失败 attempt 可能仍被上游收费，这不是响应协议可以推导的
数据；目录计价只覆盖供应商明确返回 usage 的成功请求，该残余风险必须在真实 provider
证据中记录。
