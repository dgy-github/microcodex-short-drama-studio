# HANDOFF

status: active
date: 2026-07-27
agent: Codex
branch: main
head: 04eb448
working_tree: dirty；尚未形成首个提交并 push
origin: https://github.com/dgy-github/microcodex-short-drama-studio.git

## 接手必读

依次读 `docs/ROADMAP.md`、本文档、`docs/STORY_EVAL_V1.md`。开发前运行
`python scripts/init_project.py --check`。数值以 manifest、rubric 和 judges 配置为准。

## 当前阶段

用户已要求提前开发 P4 并规划 P5-P10。主依赖链仍停在 P1；P4 只提前实现
不依赖真实 runtime 数据的基础边界。当前版本只写故事，不处理视频。

## 已完成

- 30 个原创 case 已切分；10/10 dev 基线已生成归档。
- Qwen 百炼与 GLM Ark 可用；stage-0 两轮探针已完成。
- negative 已收紧为 2 条；双 specificity 指标已实现。
- 窄 pair：Qwen all/cross `0.4444/0.5714`；GLM `0.6667/0.7143`。
- 项目初始化、记忆、OpenAPI、CI 已落地。
- 视频技术调研文档已标为 deferred，仅作未来参考，不构成当前开发任务。
- P4 的**两项可预建项**已完成，其余 4 项**全部 gated，当前无一可推进**
  （非 33% 进度）：
  - 已完成：`story-storage` 全候选决策留存接口及完整性校验（保留 t06 losers
    和分数）；`story-core::ArtifactSpanRef` 与 `story-policy::Defect.span`。
  - gated：`D5` 等 provider inventory + 冻结的 eval；`D6` 等 runtime failure
    taxonomy；`proxy_fidelity` 阻塞于 P3b 的首批留存候选；编剧招募为 external。
- P5-P10 已写入 `docs/ROADMAP.md`，全部限定为故事创作产品路线。
- **P7（专业评审）改为从 P3a 起并行**，不再串在 P6 之后。它唯一的真实依赖是
  「冻结的 rubric + 有产物」，且是唯一卡在**招人**而非写代码的阶段。串行会导致
  产品在任何合格读者看过之前就功能完备——正是整套评测设计要防的事。
  **P3a 一冻结就启动编剧采购。**
- **凭据静态加密从 P9 前移到 P5，已定（2026-07-27）**。
  **用户用的是自己的 provider key**——这不是待定项，设计已答：P10 要求
  「first-run provider configuration」且 Exit 为「干净机器安装后自行配置」，
  `ARCHITECTURE.md` 把 provider keys 判给 Rust product，且全仓库无
  billing/subscription/proxy/hosted 层——装在用户机器上、无服务端，key 只能是用户的。
  加密放 P5 的理由是**成本不对称**：当前尚无任何凭据存储代码
  （`story-provider` 只有 trait），写的时候就加密几乎不额外花钱；事后补要做
  明文迁移、双格式兼容和升级路径。且明文密钥是通过**日常开发流程**泄漏的，
  不是通过发布——本项目开发期间 key 就进过 `.env` 并差点进入被跟踪文件。
  即便 P5 最终仅内部使用也无损失，这段代码本来就要写。P9 保留轮换与审计。
- **P8 补入评测集保活**：challenge split 季度刷新（`STORY_EVAL_DESIGN` §4）与
  对抗集退役规则（`STORY_EVAL_ADVERSARIAL` §8 标记为 open）。此前两条在写阶段
  计划时丢失。固定不变的对抗集会被 Goodhart 掉——这是我们自己文档里的判断。
  这是**周期性工作，无退出条件**。

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

> ⚠️ **这张表不能当剩余工作量读，两处已知失真：**
>
> - **P1 是条件退出，不是清单退出。** 退出条件是「两个 judge 各拿到可读且稳定的
>   specificity」。窄 pair 实测 Qwen `order_consistent: False`、GLM
>   `self_consistency: 0.575`，若需引入第三个 judge 族，那不在这 6 项里。
>   **P1 在条件满足前是开放的、不可数的。**
> - **P5-P10 的 30 项是 epic，不是 task。** 「signed Windows packaging and
>   reproducible build evidence」不是一项。真实粒度展开后会显著多于 30。
>
> 因此「距离首个端到端剧情产品 17 项」是最容易误读的一行——它把条件退出的阶段
> 当成了可数清单。

| 范围 | 剩余 |
|---|---:|
| P0 封存当前状态 | 1 |
| P1 校准仪器 | 6 |
| P2.5 形态无关层 | 1 |
| P3a 冻结评测集 | 4 |
| P3b 首个端到端 runtime | 5 |
| **首个端到端剧情产品前** | **17** |
| P4 决策可观测与纠偏 | 4 |
| P5-P10 | 30 |
| **P0-P10 全部已知待办** | **51** |

### P0-P3 的 17 项明细

- P0：整理、提交并 push 当前工作树。
- P1：重算窄 pair metrics；结果输入指纹；status 门槛；处理 Qwen 位置偏置；
  `inter_model_agreement`；清理旧结果并重跑双 judge 12 次。
- P2.5：不可变 `content_form` job 契约。通用 span 已完成；修订对应移入 P6。
- P3a：剩余 9 个基线评分；维度相关矩阵；人工 spot check；冻结版本。
- P3b：sidecar 生命周期；异步事件/SSE；固定执行序；注册 agent lanes；
  授权检索后跑通并评分一个 story package。

## 下一步

P4 下一步：为 D5 模型路由建立 G0 需求和 provider 能力清单；冻结评测集前不接入
生产选择逻辑。主依赖链仍应先执行 P1-1，用窄 pair 重算 metrics（无需 API）。

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
