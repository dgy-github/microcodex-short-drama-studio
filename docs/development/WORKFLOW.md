# Guided delivery workflow

## G0 — Requirements ready

用 `REQ-*` 记录范围、排除项、优先级、风险、非功能要求和可观察验收标准。

## G1 — Experience and system design ready

完成 UI 状态、系统与信任边界、失败处理、可观测性、发布方案和 ADR。

## G2 — Contracts ready

完成 OpenAPI、事件/schema、存储设计、前后端计划、测试计划和追踪矩阵。
跨层实现只能在此门禁后开始。

## G3 — Parallel development ready

Svelte mock、Rust runtime 和 Python sidecar 从同一契约开发，不得各自发明字段。

## G4 — Implementation complete

各层单元、契约和 provider 测试通过。

## G5 — Real integration

启动本地依赖和进程，验证认证 SSE、断线续传、去重和真实流量。

## G6 — Requirement acceptance

对真实本地栈执行 P0/P1 requirement smoke tests。

## G7 — Release ready

完成回归、安全、迁移、文档、项目记忆、追踪和发布检查。

接口状态：`DRAFT -> REVIEWED -> MOCK_READY -> BACKEND_IMPLEMENTED ->
PROVIDER_VERIFIED -> FRONTEND_CONNECTED -> INTEGRATION_PASSED ->
SMOKE_PASSED -> DONE`。
