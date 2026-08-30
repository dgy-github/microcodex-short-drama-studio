# Handoff：Wan 粗生成 Provider

## 目标

为视频 Agent 完成阿里云百炼 DashScope Wan 异步任务的请求、轮询、失败、超时、取消和结果映射；保持凭据、网络、费用、artifact 和执行权在主项目 Rust trusted capability。

## 范围

只修改本任务要求列出的 Wan 契约、适配设计、Rust media gateway 适配和测试。不得把密钥或网络代码放进 Python Agent。

## 依赖与验收

无前置依赖。必须有官方文档依据和 fake HTTP 覆盖成功、失败、超时、跨源 status URL；运行 `python -m unittest discover -s tests -p "test_*.py"`，主项目适用 Rust 测试也必须通过。

## 状态

`pending`。完成后在本文件追加提交号、文档依据、测试结果和未决问题。

## 交接规则

不修改其他 parallel-tasks 目录；真实 API 证据不足时保持 fail-closed，不宣称已接通。
