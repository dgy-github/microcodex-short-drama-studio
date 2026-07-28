---
name: guided-product-development
description: 按 G0-G7 门禁推进短剧工作室的跨层功能，从需求、设计、契约、实现、联调、验收到发布。用于新增产品功能或跨 Rust、Python sidecar、Svelte 的变更。
---

# 引导式产品开发

1. 阅读开发流程、追踪规范和项目记忆。
2. 在 `docs/features/<feature-id>/` 建立功能目录并分配稳定 ID。
3. 按 G0-G7 推进；G2 契约就绪前不得开始跨层实现。
4. Rust 保持可信存储、供应商访问、权限、预算和进程执行所有权。
5. Python sidecar 只接收类型化能力；Svelte 不直连 sidecar 或模型供应商。
6. 每阶段用文件、测试、日志或报告证明门禁状态。
7. 完成前执行追踪审计和适用的仓库检查。

不要一次性用假设填满所有文档；先记录未决问题，再逐步收敛。
