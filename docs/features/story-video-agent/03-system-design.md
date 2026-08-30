# Story Video Agent design

Status: G2 contracts ready

`../story-video-agent` 是主仓库同级的独立 Python Git 项目。输入由图片的 content
address、故事 span 与 shot 参数组成，输出 `video-generation-request/v1`。它不接受本地
文件路径，也不执行 FFmpeg 或 provider HTTP。

未来 Rust media capability 复用 `story-provider` 的认证、重试与费用策略，复用
`story-storage` 的 content-addressed retention，并通过 `story-runtime` durable events
提供取消、恢复和 SSE。该项目只负责将故事因果和镜头意图转成 provider-neutral 请求，
避免复制基础设施状态机。

视频 run 与生图 run 共用 `story-media::MediaRunService` 和 `MediaEventStore`。视频执行前
还必须证明输入图片位于同一 project 的 immutable media index，伪造 content reference
或其他项目的图片都不能进入 provider。
