# 任务 02：可灵精生成 Provider

## 目标

为视频 Agent 增加可灵精生成的认证、异步任务和结果映射设计。JWT/API 凭据只能存在 Rust trusted provider，不能进入 Agent 或 UI 日志。

## 必须交付

- 固定 API host、路径、版本和模型路由配置。
- JWT/签名生成、时间窗口、nonce 和 secret 脱敏测试。
- 提交、轮询、失败、超时和取消状态映射。
- 严格绑定输入图片 artifact 和 story spans。
- fake HTTP 测试覆盖签名错误、过期、跨源轮询 URL 和重复提交。

## 写入范围

- `D:/github_dgy/story-video-agent/` 的可灵适配契约、设计和测试。
- 如需主项目 Rust 接入，只修改 `crates/story-provider/src/media_gateway.rs` 及其测试。

## 不得修改

不得修改 Wan 任务目录、存储实现或 Svelte UI。

## 验收

```powershell
python -m unittest discover -s tests -p "test_*.py"
```

没有可靠官方文档时只能提交设计和 fail-closed 测试，不得声称真实 API 已接通。
