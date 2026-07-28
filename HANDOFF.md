# HANDOFF

status: active
date: 2026-07-27
agent: Codex
branch: main
sealed_commit: 1cdc5c2
p1_evidence_commit: 4cbd809
working_tree: clean after this handoff update
origin: https://github.com/dgy-github/microcodex-short-drama-studio.git

## 接手必读

依次读 `docs/ROADMAP.md`、本文档、`docs/STORY_EVAL_V1.md`。开发前运行
`python scripts/init_project.py --check`。数值以 manifest、rubric 和 judges 配置为准。

## 当前阶段

P0 已封存。P1 工程清单完成，但稳定性退出条件失败；P2 选择“judge 不可靠”分支
并返回 P1。P2.5/P3 暂不解锁。当前版本只写故事，不处理视频。

## 已完成

- 30 个原创 case 已切分；10/10 dev 基线已生成归档。
- P0 已完成：145 个文件封存于 `1cdc5c2`，提交前完成敏感信息和大文件审计。
- Qwen 百炼与 GLM Ark 可用；窄 pair 已按同一输入指纹重跑完整12次调用。
- P1 已完成指纹、status 门槛、双 specificity、重算 metrics 和 inter-model alpha。
- 最新结果：Qwen cross/consistency `0.7143/0.925`，order flip；GLM
  `0.1429/0.475`，order flip；alpha `0.3312`；严格 pair detection `0.0`。
- 项目初始化、记忆、OpenAPI、CI 已落地。
- 视频技术调研文档已标为 deferred，仅作未来参考，不构成当前开发任务。
- P4 的**两项可预建项**已完成，其余 4 项**全部 gated，当前无一可推进**
  （非 33% 进度）：
  - 已完成：`story-storage` 全候选决策留存接口及完整性校验（保留 t06 losers
    和分数）；`story-core::ArtifactSpanRef` 与 `story-policy::Defect.span`。
  - gated：`D5` 等 provider inventory + 冻结的 eval；`D6` 等 runtime failure
    taxonomy；`proxy_fidelity` 阻塞于 P3b 的首批留存候选；编剧招募为 external。
- P5-P10 已写入 `docs/ROADMAP.md`，全部限定为故事创作产品路线。
- P7 从 P3a 起并行启动编剧采购；专业评审不等待桌面端和修订 UI。
- 用户自带 provider key；凭据静态加密前移到 P5，P9 保留轮换和审计。
- P8 包含 challenge 季度刷新和对抗集退役规则。详细理由见 `docs/ROADMAP.md`。
- P11-P16 post-1.0 登记册已写入 ROADMAP；P17-P18 刻意留空，避免虚假精度。

## 最近验证

2026-07-27 全量通过：

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`：33 项 Rust 测试
- sidecar 2 项、eval 55 项 Python 测试
- 项目记忆、registry、OpenAPI、traceability、ownership、init 检查
- `git diff --check`

## 主线剩余

计数口径：ROADMAP 中一个可独立验收动作算一项；P2 决策门不计；
Stage 1 延期项不计。视频下载和素材提取不计入项目待办。

> 计数只用于导航：P1 按条件退出，P5-P10 是 epic，不能换算成工期或完成率。

| 范围 | 剩余 |
|---|---:|
| P0 封存当前状态 | 0 |
| P1 校准仪器 | 条件阻塞 |
| P2.5 形态无关层 | 1 |
| P3a 冻结评测集 | 4 |
| P3b 首个端到端 runtime | 5 |
| **首个端到端剧情产品前** | **16** |
| P4 决策可观测与纠偏 | 4 |
| P5-P10 | 30 |
| **P0-P10 全部已知待办（首个可发布产品）** | **50** |
| P11-P16 post-1.0 登记册 | 不计数 |

### P1-P3 的 16 项明细

- P1：重算窄 pair metrics；结果输入指纹；status 门槛；处理 Qwen 位置偏置；
  `inter_model_agreement`；清理旧结果并重跑双 judge 12 次。
- P2.5：不可变 `content_form` job 契约。通用 span 已完成；修订对应移入 P6。
- P3a：剩余 9 个基线评分；维度相关矩阵；人工 spot check；冻结版本。
- P3b：sidecar 生命周期；异步事件/SSE；固定执行序；注册 agent lanes；
  授权检索后跑通并评分一个 story package。

## 下一步

为 P1 增加一个非中文原生第三 judge，或完成内部人工 blind spot check。现有
`.env` 只有 Qwen/GLM 凭据；获得新证据前不得进入 P2.5/P3。

## Do-Not

- API key 只放 `.env`；聊天中出现过的百炼 key 应轮换。
- Ark 用 `glm-5-2-260617`，不用裸名 `glm-5.2`。
- 不复用缺少输入指纹的旧 judge result。
- 不把 `measurable_gap` 当准入通过，不比较低稳定 judge 的单点 specificity。
- P2.5 完成前不新增其他 content form 的 rubric/case/template。
- 不复制 nanocodex 的阻塞调用或独立 SQLite owner。
- 不让 Svelte/Python 直接执行 FFmpeg、访问 provider key 或可信存储。
- 不把 Douyin 抓取、cookies、反爬或未授权素材纳入视频素材 MVP。
- 当前版本不创建媒体 schema、`story-media` crate、FFmpeg 依赖或素材 UI。
