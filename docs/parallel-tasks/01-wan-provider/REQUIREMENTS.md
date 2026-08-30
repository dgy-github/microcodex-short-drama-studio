# 任务 01：Wan 粗生成 Provider

## 目标

为视频 Agent 增加真实 Wan（阿里云百炼 DashScope）异步任务的 provider-neutral adapter 设计和可测试实现。密钥、网络调用、轮询、取消、费用和 artifact 存储必须仍由主项目 Rust capability 负责；Python Agent 只产生类型化请求。

## 必须交付

- 明确 DashScope Wan 请求/响应字段映射。
- 支持提交、轮询、失败、超时和取消状态映射。
- 拒绝非 HTTPS、跨源轮询 URL、缺少任务 ID 和未知状态。
- 记录 provider、模型、请求 ID、成本字段，不记录 secret。
- 使用 fake HTTP 测试覆盖成功、失败、超时和恶意 status URL。

## 写入范围

- `D:/github_dgy/story-video-agent/` 的 Wan 适配设计、契约和测试。
- 如需主项目 Rust 接入，只修改 `crates/story-provider/src/media_gateway.rs` 及其测试，并同步契约。

## 不得修改

不得修改 `story_image_agent`、桌面 Svelte、存储所有权或 Python 网络权限边界。

## 验收

```powershell
python -m unittest discover -s tests -p "test_*.py"
```

## 完成记录

在本目录创建 `STATUS.md`，记录提交号、测试输出摘要和真实官方文档依据。
