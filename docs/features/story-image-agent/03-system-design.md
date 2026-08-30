# Story Image Agent design

Status: G2 contracts ready

`projects/story-image-agent` 是独立 Python orchestration workspace。它拥有图片提示词规划与
revision 因果关系，但不拥有 provider、凭据、计价、重试、预算、事件存储或 artifact
存储。生成请求遵循 `image-generation-request/v1`，由未来的 authenticated Rust media
capability 执行并返回 `artifact://sha256/...`。

流程为：story package span → 初始 prompt revision → 用户编辑产生子 revision → typed
generation request → Rust provider → immutable image artifact。重新生成只追加 request 和
artifact，不修改任何既有记录。取消、恢复和进度复用 `story-runtime` 的命令与 durable
event 模型，不创建第二状态机。

`story-media::MediaRunService` 现提供 Rust-owned durable lifecycle：accepted、started、
completed/failed/cancelled、recovered；同一 run 只接受一次且只有一个 terminal。取消会
drop 未完成 provider future，恢复只枚举有 accepted、无 terminal 的 request。

`story-storage::MediaProjectRepository` 持久化 prompt revision 与 generation request。Python
workspace 的后续 adapter 必须调用这个 typed 边界，不能把 revision history 作为进程内唯一
事实源。
