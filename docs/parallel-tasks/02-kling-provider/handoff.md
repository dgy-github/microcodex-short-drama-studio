# Handoff：可灵精生成 Provider

## 目标

为视频 Agent 完成可灵精生成的认证、异步任务、状态、错误和结果映射设计；JWT/API 凭据只能由 Rust trusted provider 持有。

## 范围

只修改可灵契约、适配设计、主项目 media gateway 对应适配和测试。不得修改 Wan 任务、存储实现或 Svelte UI。

## 依赖与验收

无前置依赖。必须覆盖签名错误、过期、重复提交、跨源轮询 URL、失败和取消；官方依据不足时只提交设计和 fail-closed 测试。

## 状态

`pending`。完成后追加提交号、官方依据、测试结果和未决问题。
