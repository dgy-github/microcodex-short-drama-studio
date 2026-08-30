date: 2026-08-27
agent: ZCode (GLM-5.3)
branch: feature/eval-p3a-unlock
origin: https://github.com/dgy-github/microcodex-short-drama-studio.git

## 最新改进 (2026-08-27) · 评测体系补完（P1/P3a 解锁线）

## 最新改进 (2026-08-30) · 结构清理继续

### 媒体 Agent 生产流水线（2026-08-30）

- 生图 Agent 最新 `ec350ee`、生视频 Agent 最新 `36346b3` 已将质量门禁改为
  fail-closed：缺少任一必需指标或低于阈值时禁止定稿/精生成，并返回失败指标及
  建议回退阶段。

- 独立生图 Agent 已增加候选批量生成、质量评估门禁和定稿请求：`d2a1037`。
- 独立生视频 Agent 已增加粗生成、裁剪、补段、质量评估、精生成计划：`f29b08d`。
- 推荐路由为 Wan（阿里云）负责低成本粗生成，Kling/可灵负责评估通过后的精生成。
- provider 凭据、费用、重试、执行和产物保留仍由 Rust trusted capability 所有；Python
  仅输出类型化计划。
- Desktop 媒体工作区已按 BugleCat 的深色面板、蓝绿强调色、状态徽标和流程节点风格
  重构；`svelte-check` 0 errors / 0 warnings，Vitest 141 passed。
- 尚未完成的外部联调：真实 Wan/Kling provider、视频裁剪/拼接执行、质量模型校准和
  真实费用 soak，需要 provider 凭据及 Rust provider 接入后验收。
- 两个独立 Agent 的 GitHub Actions 已修正发布配置：显式限制 setuptools 包发现，
  并安装 `.[test]` 测试依赖；最新 CI 修复提交分别为生图 `e9491bc`、生视频
  `bddbaf8`，等待新 run 结果确认。

- 修复项目记忆与结构门禁对本地工具环境的误扫描：`.release-venv`、`.mimosa`、
  `.workbuddy`、`.zcode` 及打包 sidecar 依赖树现在明确排除；新增项目记忆回归
  覆盖。项目记忆测试 25/25、`generate_project_memory.py --check` 和全量结构检查
  172 个文件均通过。此前报告的第三方依赖超长文件不再污染“存量清单”。
- 本次回归验证：sidecar 41/41，eval/tools 150/150，图片/视频 Agent 5/5，
  Rust workspace 测试通过，clippy `-D warnings` 通过；桌面 Vitest 141/141、
  `svelte-check` 0 errors/0 warnings、Vite production build 通过。
- 当前工作树仍保留本机 `.release-venv` 临时目录；删除操作受本机会话安全策略拦截，
  但所有源码/项目记忆/结构扫描器均已忽略它。`npm audit` 受 npmmirror 未实现
  security audit endpoint 阻断；此前 `npm audit --omit=dev --omit=optional` 在可用
  registry 下的结果仍为 0 个生产漏洞。
- 重新从评测 manifest 生成并校验 `docs/eval-governance.html`，修复了治理页生成物
  漂移；`generate_governance_page.py --check` 现已通过。评测 freeze 仍不创建虚假
  记录，缺失的人工证据继续作为外部门槛保留。
- CI 治理输入门禁复核通过：120 个案例有效，拆分文件与 assignment table 一致，
  premise family 交叉近重复为 0，治理页检查通过。freeze 检查仅剩人工证据缺失，
  按范围不将其冒充为自动化缺陷。
- GitHub 公共 Actions API 显示远端最近一次 Governance（run `31712246842`，旧提交
  `37c9f06`）仍为失败：governance/init、python eval/tools、Rust clippy 和真实 Tauri
  E2E 四个 job 失败。当前工作树已逐条重跑这些失败路径并全部通过：init/governance、
  eval/tools 150/150、workspace clippy `-D warnings`、Tauri WebView2 IPC 4/4。由于
  当前改动尚未形成并推送对应提交，不得把本地通过描述为远端 CI 已绿。

- Desktop 评价服务的评分记录构造已拆到 `apps/desktop/src-tauri/src/evaluation_scoring.rs`，
  32 个非付费测试通过，1 个真实付费 workflow 测试按设计 ignored。
- stage-0 探针的 JSON 响应解析已拆到 `eval/tools/probe_parsing.py`，保留原 CLI 行为；
  eval/tools 测试 149/149 通过。
- 当前结构扫描剩余 `eval/tools/run_stage0_probe.py`（约 1074 行）。它仍需按
- transport、judge validation、probe orchestration 三个职责继续拆分；本轮已将
  transport 与 judge validation 拆为 `probe_transport.py`、`probe_judging.py`，
  主脚本现已低于 700 行，结构扫描全量通过。
- npm audit 已完成一次安全的非破坏性修复：Vitest 升至 4.1.11，critical 漏洞清零，
  `npm audit fix` 可处理项已应用。当前仍有 13 个 high 级开发依赖告警，全部来自
  WDIO 9/Tauri service 传递链；npm 建议降级到 8/7，属于破坏性变更，且 Tauri
  service 没有可用修复版本，需单独安排 WDIO/Tauri 兼容性升级和真实 E2E 回归。
  `npm audit --omit=dev --omit=optional` 的发布依赖结果为 0，且已接入
  desktop-windows CI，避免生产供应链风险被已知的测试工具告警掩盖。
- 修复 stage-0 历史评测产物漂移：`motive-explicit/evaluator-metrics.json` 不再声称
  inter-model agreement estimator 未实现，已由当前计算器从磁盘 judge 样本重生成，
  Krippendorff interval alpha 为 0.51636（2 个判官、20 项）；人工 spot check 仍按
  项目边界明确标为不可计算。
- WDIO Tauri 配置已从已弃用的 `driverProvider: "official"` 迁移为
  `driverProvider: "external"`；依赖升级后真实 Tauri WebView2 IPC E2E 已复跑通过，
  4/4 用例通过。tauri-service 仍输出窗口状态探测的 404 warning，但不影响测试断言，
  应在后续 service 版本升级时继续跟踪。
- 本机已实际尝试 unsigned Windows release pipeline，但在构建前被可复现工具链门禁
  拒绝：脚本要求 Node v22.14.0，本机为 v22.21.1，且本机 nvm 无已安装的 22.14.0。
  这不是 MSI/NSIS 构建失败；不得放宽脚本版本约束。`windows-release-smoke` CI 已固定
  Node 22.14.0，需由 workflow_dispatch 或具备该版本的 clean 环境补齐 bundle smoke 证据。
- 已修复 release 的实际 bundle 缺陷：NSIS hook 所需的 x64 `WebView2Loader.dll` 原先
  未从 Cargo build 输出复制到 `target/release`，导致安装器构建失败；release 脚本现在
  在 Tauri bundling 前执行 release Cargo build、要求唯一 x64 loader 并复制到约定位置。
  使用官方 Node 22.14.0 与 Python 3.12.10 重跑后，MSI/NSIS 双包、MSI 提取、sidecar
  协议、WebView2Loader、story schema 和桌面启动 smoke 全部通过。证据在
  `target/release-evidence/windows-{bundle-smoke,release-evidence}.json`；产物 dirty/unsigned，
  仅为本地验证，不具备稳定发行资格。

REQ-320..326（`docs/features/eval-p3a-unlock/`），三笔提交：
`9ead9e0` 工具链、`184b951` stage-1 对抗对、`72efc51` 120 案。

### 已完成
- ✅ 逐案打分管线 `eval/tools/score_artifacts.py`：归档 baseline × 判官 × 3 采样，
  `eval-score-record/v1` 输出到 `eval/scores/<run-id>/`（scores.jsonl + 逐判官结果），
  六文件输入指纹、断点续跑、复用/失效判定（REQ-320）
- ✅ 维度相关矩阵 `compute_pillar_review.py`：Spearman 10×10 + manifest 阈值并柱建议，
  `pillar_grouping_review` 等待的证据可产出（REQ-321）
- ✅ `compute_spot_check_agreement.py`：桌面盲测人评 × 判官分按 artifact_id 连接，
  逐产物完整块 nominal alpha + 逐维判官-人均差（REQ-322）
- ✅ `eval-governance.html` 改为生成物（模板 + manifest/仓库状态注入），`--check` 入
  governance CI；修复三处陈旧漂移（REQ-323）
- ✅ 冻结机制：`eval/manifests/FREEZE.json` + `scripts/check_eval_freeze.py` 入 CI，
  冻结后哈希漂移即 fail（MAJOR bump 语义）（REQ-324）
- ✅ stage-1 七路 masking 探针**创作侧**：六路定向降级同底 comedy_002
  （HOOK_FAKE/FALSE_PAYOFF/EMOTION_UNEARNED/VOICE_COLLAPSE/PLOT_CONVENIENCE/TROPE_STACK），
  全过准入门、字节级改动断言、char delta ≤0.51%（REQ-325）
- ✅ 案例集 **30→120**：90 个内部原创案，父契约配额 ×4，30:30:24:12（dev 38/train 37/
  validation 30/challenge 15），holdout 保持 0 封存，53 个新 premise family 零跨 split（REQ-326）
- ✅ 修复：`load_probe_config` 补充判官与正式判官重复计数（虚高 agreement）；
  `archive_baselines` 哈希换行归一化（修 Windows autocrlf 下 21 个误报）；
  pair schema 补 FALSE_PAYOFF 码与 rationale 字段

### 测量被凭证阻塞（非工具问题）
2026-08-27 实测：三条判官路全部不可用——
1. `JUDGE_API_KEY`（Qwen/阿里百炼）当前环境未设；
2. GLM 智谱/火山两条路由均欠费（历史记录）；
3. 本地 codex（gpt-5.4）默认中转 HK-CLIProxyAPI 返回 401，
   `--config model_provider=openai` 走 auth.json 的 sk- key 也被 401 拒绝。

### 测量已执行 (2026-08-27 晚，抽样)
判官 glm-5.3-flash + gpt-5.4 经 teamorouter 中转（qwen/glm-5.2 路由因成本/速度暂停，见 judges.json blocked_on 注记）。

**stage-1 六对探针（采样=2/序，48 次调用）——seeded_defect_detection = 0/6（目标 0.75）：**
六条 masking 配方全部存活。逐对检出率（negative_lower）：
hook-fake glm 1.0/gpt 0.5；false-payoff **0.0/0.0（最强存活者，双判官全盲）**；
emotion-unearned 1.0/0.5；voice-collapse 1.0/0.5；plot-convenience 0.5/0.5；trope-stack 1.0/0.5。
gpt-5.4 在 6 对中 5 对恰为 0.50（n=4 下顺序噪声分不开）；glm-5.3-flash 显著更敏感。
inter_model_agreement = 0.519（120 项）。defect_localisation = 1.0 但判官引用 span 分散、精确率低，命中有兜底嫌疑。
窄对（motive-explicit-narrow）为七月旧判官配置的历史测量，指纹已随 judges.json 演进失配，未混入本期 headline。
报告：`eval/adversarial/evaluator-metrics.json`。

**逐案打分（5 案抽样 × 2 判官 × 3 采样，30 份记录）：** `eval/scores/baseline-20260827/`。
pillar review 首读已产出（`pillar-review-10records.json`）：5 案下相关矩阵以 NaN/伪 1.0 为主，
caveat 已标注判官案数 <10——**需要补满 10 案才有可用结论**（每补 1 案 ≈ 6 次调用）。

**解读**：新证据再次确认 P1"判官稳定性差"。§9 分支决策（40 对量产）暂缓——判官读不出差异时，
量产对抗对测的是噪声。先补打分到 10 案 + 人工盲测（你本人）交叉验证判官是否系统性偏高。

### 盲测开箱清单（2026-08-27 就绪）
`python eval/tools/plan_spot_check.py` 已生成确定性抽样计划
（`eval/scores/spot-check-plan.json`）：29 案（9 题材全覆盖）+ 8 个对抗对。
桌面 EvaluationCenter → offline-v0.1.0 → 人工盲测 → 按计划勾选 → 逐份十维评分 →
`python eval/tools/compute_spot_check_agreement.py --runs eval/scores/baseline-20260827`。
### P11 MinHash 查重（2026-08-27 完成）
120 案机器查重通过：跨家族近重复 0（阈值 0.5，3-gram + MinHash128 + 精确 Jaccard 复核）；
25 个家族成员间零表文重叠（机制级家族的预期形态，记录为信息级）。

### 下一步优先级（判官路已恢复，此段为历史）
1. ~~**恢复任一判官路**~~ 已完成（glm-5.3-flash + gpt-5.4 经 teamorouter）：
2. 🔴 **P1 人工盲测（你本人）**：桌面端 EvaluationCenter → 人工盲测 → 选 offline
   数据集抽样（manifest 20% + 全部对抗对）创建分派并逐份评分；随后
   `python eval/tools/compute_spot_check_agreement.py --runs eval/scores/<id>`
   （本机桌面尚无 evaluation 目录，需先跑一次桌面端建目录）
3. 🟡 打分齐 30 份记录后：`compute_pillar_review.py --runs ...` → 复核结论
4. 🟡 三族齐 → 重算 evaluator-metrics → stage-1 分支决策（≥4 路有效→40 对）
5. 🟢 冻结（等 2+3 完成）：写 `FREEZE.json`（hash + 证据链接），VERSIONS.md §4 置 frozen
6. 🟢 GLM 充值恢复第三族；编剧面板（P7）仍是纯外部依赖

## 接手必读

依次读 `docs/ROADMAP.md`、本文件、`docs/SECURITY_REVIEW.md` 和
`docs/STORY_EVAL_V1.md`。

## 历史记录 (2026-08-10)

由 Claude Fable 5 完成的代码质量提升工作：

### 已完成
- ✅ 删除孤儿 `templates/` 目录（已被 `config/genre-packs/` 取代）
- ✅ 修复测试输出误导：重试日志改为 stderr
- ✅ 创建 `TROUBLESHOOTING.md` - 常见问题和解决方案
- ✅ 创建 `docs/CLEAN_VM_ACCEPTANCE_TEST.md` - P10 验收脚本
- ✅ 创建 `scripts/setup_dev_environment.py` - 自动化环境设置
- ✅ 创建 `docs/RELEASE_CHECKLIST.md` - 发布检查清单
- ✅ 更新 `README.md` - 添加快速开始指南
- ✅ 代码质量评估完成 - 总体评级 A-（单人开发背景下罕见的工程成熟度）

### 质量评估结论
**评级**: A-（优秀，有待改进）

**优势**:
- 架构设计优秀（A+）：边界清晰、fail-closed、形式无关抽象
- 安全实践到位（A）：凭据加密、零泄露、诊断脱敏
- 文档完整详尽（A）：包含设计文档、ADR、审计报告
- 工程纪律严格：明确的 Exit 条件、版本化合约、诚实面对问题
- 自我纪律异常严格（罕见于单人项目）

**待改进**:
- P1 退出条件失败：seeded_defect_detection = 0.0（目标 0.75），但该值来自
  `pairs_total = 1`，只有一个窄降级对，取值只可能是 0.0 或 1.0
- P10 Clean VM 验收未执行
- 可复现性问题（已通过新文档改善）
- 测试覆盖不均（核心充分，桌面端较少）

### 下一步优先级
1. 🔴 **P10 验收** - 执行 `docs/CLEAN_VM_ACCEPTANCE_TEST.md`
2. 🔴 **P1 人工盲测** - 解除 advisory/non-promotable 状态
3. 🟡 **充值 GLM** - 恢复第三个 judge 族
4. 🟢 **桌面端测试** - 补充集成测试（可选）

## 接手必读

依次读 `docs/ROADMAP.md`、本文件、`docs/SECURITY_REVIEW.md` 和
`docs/STORY_EVAL_V1.md`。

```powershell
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -e sidecar
.\.venv\Scripts\python.exe -m pip install -r scripts/requirements.txt
.\.venv\Scripts\python.exe -m pip install -r eval/tools/requirements.txt
.\.venv\Scripts\python.exe scripts/init_project.py --check
```

全量验证四条缺一不可；桌面端自带独立 workspace：

```powershell
cargo test --workspace --all-features
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
.\.venv\Scripts\python.exe -m unittest discover -s sidecar -p "test_*.py"
.\.venv\Scripts\python.exe -m unittest discover -s eval/tools -p "test_*.py"
```

当前产品只写故事，不下载视频、不提取素材、不自动发布。

## 当前结论

### 2026-08-30 多 Agent 可靠性与媒体 Agent 基础

- 原 `sidecar/campaign_adapter/workflow.py` 已按 graph、agent、context、capability、
  prompt、packaging、runner 拆为 8 个模块；公共 import 保持兼容，结构门禁逐文件通过。
- sidecar 增加 artifact retention/recovery、跨恢复 token、整轮 deadline、并发失败取消，
  并验证已 terminal 的 run 不会被 `recover_incomplete()` 重新执行。
- 生图与视频 Agent 已迁移为主仓库同级的独立 Git 项目：`D:/github_dgy/story-image-agent`
  与 `D:/github_dgy/story-video-agent`。前者支持 append-only prompt revision 和重新生成 request；
  后者将 immutable image artifact 与 story span/shot intent 绑定成 typed request。
- 媒体链路当前通过 provider-neutral gateway 产生并持久化 schema-valid request、durable
  event 与 content-addressed media artifact；测试使用 loopback fake gateway，只证明协议和
  跨层执行，不得描述为已经由真实供应商生成图片或视频。
- `max_cny_fen` 已接入版本化 Rust pricing catalog、capability usage、sidecar 恢复与
  超限门禁、desktop 去重投影。未知 provider/model、缺目录和溢出均 fail-closed；仓库
  不硬编码未经核验的实时价格。生产前需按
  `config/provider-pricing-v1.example.json` 核验并创建 `config/provider-pricing-v1.json`。
- 计价覆盖成功响应中供应商明确返回的 usage；失败 retry attempt 若供应商收费但不返回
  usage，客户端无法推导，仍需通过供应商账单/真实 soak 记录残余偏差。
- 新增 `crates/story-media` trusted execution seam：严格校验 image/video request，要求
  视频图片在同一项目的 immutable media index 中真实存在，再调用 typed provider 并写入
  binary content-addressed store。fixture 已验证 image→video 链路和伪造 URI 拒绝。
- 媒体 run 已有 Rust durable JSONL events、全局连续 seq、单 terminal 仲裁、重复 start
  拒绝、watch cancellation 和 restart recovery。取消慢 provider 不落媒体 artifact；
  completed/failed/cancelled run 不会被重新恢复。
- 新增 `story-storage::MediaProjectRepository`，将图片 prompt revision、父 revision、
  来源 span 和 generation request 以 append-only JSONL 持久化；重开仓库恢复历史，重复
  revision/request 与缺失父 revision fail-closed。下一步需让 Python image workspace
  通过正式 typed adapter 使用它，不能继续以进程内列表作为生产事实源。
- 生图 workspace 的 `RustMediaProjectClient` 已接通 Rust capability host 的
  `/v1/media-projects/records`：复用同一 bearer token，只允许 typed prompt revision / generation
  request，并由 Rust 派生可信存储根。未认证、错误 schema、缺父 revision 与重复记录均
  fail-closed；桌面媒体项目 UI 已接入，真实图片 provider 仍待外部配置和验证。
- `story-provider` 的媒体 history adapter 与 story-package validation 已从
  `capability_host.rs` 拆到独立模块，host 文件重新低于 700 行结构门禁。
- `story-media` 仍只有 Rust-owned executor/run/event/storage 与 fixture 集成，桌面端尚无
  独立生成执行命令。桌面端现已新增常驻媒体历史服务及三个 typed IPC：追加 prompt
  revision、追加 image/video generation request、读取项目历史；它们直接复用 Rust
  `MediaProjectRepository`，不依赖故事 run。现有 capability host 生命周期仍绑定故事 run，
  真正出图/出视频前需新增独立 media execution runtime 和真实 provider adapter。
- 已新增 provider-neutral `media-gateway-response/v1` 契约、Rust authenticated gateway client
  和 `story-media::GatewayMediaProvider` 适配器；真实 loopback HTTP 测试验证了鉴权、schema、
  Base64 二进制和媒体元数据校验。它仍不绑定具体供应商，生产部署需由运营配置并验证实际
  gateway endpoint、凭据、价格和返回格式。
- Desktop credential owner 已允许 `media_gateway/default`，secret 仍只进入 Windows Credential
  Manager；新增独立 media gateway settings 服务与读取/保存 IPC，仅持久化经 Rust 校验的
  HTTPS `/v1/media/generate` endpoint。独立 execution runtime 可直接从这两个 owner 组装
  `MediaGatewayRoute`，无需让前端或 Python 接触 token。
- Desktop 已新增独立 `DesktopMediaRuntime` 与 start/resume/cancel IPC：它从 credential/settings
  owner 组装 `GatewayMediaProvider`，通过 `MediaRunService` 写 durable events 和 immutable
  media artifact，并在任何网络/密钥访问前要求 generation request 已存在于可信项目历史。
  当前测试覆盖未持久化请求拒绝、active run 取消身份，以及完整
  desktop→fake gateway→artifact 图片到视频链路；真实生产 gateway 验证仍待补齐。
- Media gateway route 现在允许仅限 `127.0.0.1/localhost` 的 HTTP，以支持本机受控 gateway
  进程；所有非 loopback endpoint 仍强制 HTTPS，URL 凭据、query 和 fragment 继续拒绝。
- 修复 gateway adapter 跨层序列化：此前 `MediaRequest` 会发送 `{"Image":{...}}` 枚举
  包装，gateway 契约无法识别；现在直接发送 image/video contract object。desktop →
  loopback fake gateway → `MediaRunService` → content-addressed artifact 集成测试已通过，
  并确认 `run.completed` durable event 与项目图片 digest 可校验。
- 前端 `types.ts`/`api.ts` 已覆盖 gateway settings、prompt revision、media project history、
  start/resume/cancel 和 generation result；同时将 `ChatProvider` 与通用 credential provider
  分开，避免 `media_gateway` 被错误送入 chat route/health/soak 面板。媒体工作室页面和主导航
  已落盘，支持 gateway 配置、提示词版本、图片/视频请求、取消及最近 append-only history；
  `svelte-check` 0 错误、141 个 Vitest 全绿。
- `MediaEventStore` 已改为 open 时完整校验一次并构建 acceptance/terminal 内存索引，追加
  只执行 append+fsync，不再每次重读全日志；250-run 重开、cursor replay 和单 terminal
  仲裁已验证。`MediaProjectRepository` 也在首次访问项目时完整校验并构建 record/parent
  索引，后续 revision/request 只 append+fsync；250 版连续提示词历史的重开、父链校验与
  重复拒绝已验证。跨进程并发写入锁和独立常驻 media runtime 仍是后续集成工作。

- P5-P10 工程实现已完成；P10 clean-Windows Exit 尚未满足。
- 2026-08-30 复扫：全仓共有 154 个受管源文件、97 份 Markdown 文档；持续拆分后
  `scripts/check_code_structure.py --all` 当前命中 2 个文件级存量（桌面评测服务与
  stage0 评测脚本）；函数级和测试 fixture 存量已清零。`run_controller` 测试已外移，
  `evaluations.rs` 测试已外移但生产实现仍需按评分、admission、盲测存储边界继续拆分。
  workflow、sidecar server、
  story-storage export、EvaluationCenter、ArtifactBrowser、genre-pack 与
  release-configuration 校验器已退出清单。
  `scripts/check_code_structure.py --all`
  可显式审计全量存量，默认无参数仍只检查 CI 传入的改动文件；剩余结构拆分是后续工程清理，
  不应被“0 个默认参数文件通过”误报为全仓清零。
- 本轮移除了作品库中没有后端支持的“批量删除”假入口；故事产物继续遵循不可变/append-only
  规则，当前批量操作只提供已验证的多格式导出。
- 当前版本是 `0.1.0-alpha.1`、advisory/non-promotable，不是稳定版。
- 用户允许人工盲测后置；draft pack、模型、prompt、graph、policy 和 skill
  均不得宣称 promoted。
- 最新 NSIS 已覆盖安装；桌面端重复启动保护包含 UI、前端函数和 Rust 启动锁。

## 最新付费证据

- 2026-07-30 完成真实付费 6 集固定流程：
  `run_6e1b11e1eb2747f89b544fa4f571448c`。
- 结果：17/17 tasks、5/5 reviews、`run.completed`。
- Token：154,628 / 180,000，余量 25,372；预算上限 ¥12.00。
- 产物：
  `artifacts/advisory-runs/run_6e1b11e1eb2747f89b544fa4f571448c/workflow-result.json`
  （69,724 字节）。
- t10 六个分集子 Agent 并行完成；t15 精简上下文后使用 9,244 Token，
  未再触发 `token_budget_exceeded`。
- 该次运行证明真实 DeepSeek 生成、百炼审查、17-task DAG、Schema 校验和
  成功落盘可协同完成。

## 本轮已修

- 接入项目原创 human-writing profile：t07 人物声音、t10 潜台词与动作、
  t12 人味审查、t15 证据化修订、t16 终审。
- sidecar 接受并严格校验 `human_writing` 五个任务指令。
- t15 没有场景 finding 时不重复输入/输出六集正文，由运行时保留 t10 scenes。
- 失败运行写 `run-failure.json`；首个失败原因不被连带错误覆盖。
- 失败码区分 `final_review_rejected`、`artifact_validation_failed`、
  `capability_timeout` 与 `provider_or_task_failure`。
- 故事运行中按钮显示“任务运行中 · 已防重复”；`accepted/running` 均不可再次启动。
- 作品库按真实完成时间排序；最新故事置顶并显示“最新生成”、梗概、时间和短 run ID。
- 新任务完成后自动进入作品库并选中对应故事，解决创作 run 与故事卡片脱节。
- 完整故事阅读器改为卡通漫画风：动态角色头像卡、分镜面板、镜头旁白框、
  左右角色对话气泡和潜台词标注。
- 修复新故事打包时 speaker 全部指向 `ch-1`；现在按 t10 角色姓名映射角色引用。

## 已完成能力

- Tauri 2.8 + Svelte 5；Windows Credential Manager；DeepSeek/百炼配置和健康检查。
- Start/Sync/幂等 Cancel、SSE `Last-Event-ID`、预算与事件投影。
- artifact 浏览、完整故事阅读器、双击详情、修订/审批/比较/回滚/导出。
- 离线/在线评测、人工盲测入口、Codex 第三 judge。
- MSI/NSIS、PyInstaller onedir sidecar、许可证清单和本地 bundle smoke。

## 尚未完成

- 在完整故事阅读器人工检查本次六集正文，重点确认角色对白可明显区分。
- clean Windows VM：安装→配置→完整故事→批准导出→升级→回滚。
- 推送并取得 `windows-release-smoke` clean runner 绿灯。
- 付费 soak、Qwen 批次、专业编剧双人 review/adjudication、人工盲测。
- P1 judge 稳定性仍失败；`seeded_defect_detection = 0.0`，目标 0.75。该值定义在
  pair 上而 `pairs_total = 1`，`evaluator-metrics.json` 自带 resolution_warning：
  单对时只能取 0.0/1.0，不是估计量。宽降级集 `motive-explicit` 为 1.0。
  真正的堵点是对抗集规模，不是判官能力。
- GLM 智谱/火山路由的外部账户状态仍未重新验证。
- P11-P16 尚未实现；详见 `docs/ROADMAP.md`。

## 下一步

用下一次新生成故事验证 speaker 映射和漫画气泡；当前已生成包中的旧 speaker 引用
无法可靠反推，不应伪造修复。随后记录角色混声、工具化对白和缺乏潜台词的具体 span。

## Do-Not

- 不使用聊天中出现过的 API key；只使用 Credential Manager 中轮换后的凭据。
- 不让 Svelte/Python 持有 provider key、可信存储或不受限 shell。
- 不绕过 `Last-Event-ID`、幂等键、durable event、审批或 revision 历史。
- 不把断线当任务失败；失败先读 `run-failure.json` 的精确错误码。
- 不恢复 PyInstaller onefile；不创建视频 schema、FFmpeg 依赖或素材 UI。
- 不把 unsigned、未 clean-VM 验收的包描述为公开稳定发行。

## 历史记录 (2026-08-12 · 全仓代码评审)

## 代码评审 (2026-08-12)

由 Claude Opus 5 执行的全仓扫描。以下结论均经实际运行验证，非仅阅读文档。

### 规模

| 维度 | 数据 |
| --- | --- |
| 架构 | Rust workspace(6 crates) + Python sidecar + Tauri 2/Svelte 5 桌面端 |
| 代码量 | Rust ~11k 行（含 src-tauri 4.3k）、Python ~2.7k、前端 ~5.2k、eval 4.8k |
| 文档 | 89 个 md（清理 26 份过程报告后），设计文档与过程快照已分离 |

### 总评

架构与安全设计 8/10，可验证性 7/10（本轮从 3/10 提升），文档纪律 4/10。
设计意识一直很强（评估/策略分离、事件溯源、provider 契约校验），本轮补上的是
"这些设计有没有东西在验证它"——此前的问题不是没测试，而是测试存在却不执行。

### 优势

- 分层不是堆出来的。`crates/` 按 core/eval/policy/provider/runtime/storage 切分，
  契约在 Rust、编排在 sidecar、UI 在 Tauri，边界清晰；README 的
  "异步命令 → append-only 事件 → SSE → 可重放"在 `run_protocol.rs`、`sidecar.rs`
  里确实落到了代码。
- 安全细节有专业水准。`crates/story-provider/src/openai_compatible.rs` 的
  `ProviderRoute::validate` 强制 https、拒绝 URL 内嵌凭据/query/fragment、校验路径
  后缀与长度上限；`Debug` 手写为 secret 打 `[REDACTED]`；密钥走 Windows Credential
  Manager。全仓扫描无硬编码密钥。
- 评估体系是本项目最有价值的部分：离线评估（门禁）与在线策略（决策）分离、依赖
  单向、版本号绑定文档。README 顶部老实标注 advisory，不吹稳定性。
- `cargo test --workspace` 81 passed / 0 failed。

### 当前实测状态

| 层 | 结果 |
| --- | --- |
| `cargo test --workspace` | 81 passed |
| 桌面端 `cargo test`（src-tauri） | 26 passed |
| `npm test`（vitest） | 139 passed / 0 skipped |
| `npm run test:e2e:tauri`（wdio 真实 IPC） | 4 passing |
| Python：sidecar / eval/tools / scripts | 28 / 75 / 20 passed |
| 治理脚本（init/registry/traceability/openapi/owners/release） | 全过 |

### 本轮修掉的问题

1. **桌面端 Rust 测试此前在 HEAD 上编译不过**（`CommandError` 无 `Display`）。
   修完后另有三处：artifacts list 夹具用了 `released/promotable`，被
   `parse_projection` 按设计拒绝（该函数强制 advisory/non-promotable，正是本项目
   的 alpha 不变量）；missing 用例未建作品库根目录，撞的是 `artifact_unavailable`
   而非 `artifact_missing`；run_controller 夹具指向被 gitignore 的 `eval/runs/`。
2. **e2e 二进制跑在开发模式。** `src-tauri/Cargo.toml` 缺 Tauri 模板本该有的
   `custom-protocol` feature，`cargo build` 产出的应用去连 `devUrl` 而非加载
   `../dist`，窗口显示 `ERR_CONNECTION_REFUSED`。`build:e2e:tauri` 现已启用该
   feature。
3. **CI 在干净检出上三个 job 必挂**：`scripts/requirements.txt` 等被引用却未跟踪；
   desktop-windows 从不创建 `.venv`，而桌面端测试要靠它拉起真实 Python sidecar。
4. **Playwright 套件是死代码**：`hasTauriWebViewDriver` 硬编码为 `false`，25 个
   用例永久 skip，从未执行过。场景已由 wdio 真实 IPC 覆盖，故整套删除。
5. CI 新增 `structure` job，对本次改动的文件执行 `check_code_structure` 大小限制。

### 记录可信度的教训

提交 `5d488fd` 写"19/19 通过"，而那批用例从未真正执行；`TODO_REMAINING.md` 称
pytest 收集 0 个用例，实测 25 个。**本仓库的历史文档和提交信息不能直接当证据用。**
门禁补上后这类漂移会被自动拦截，但存量文档里还有多少失真没有核过。

### 尚未处理

- 19 处结构违规存量：`evaluations.rs` 1221 行、`run_controller.rs` 868 行、
  `revisions.rs` 804 行、`ArtifactBrowser.svelte` 597 行等（新代码已被门禁拦住）。
- `npm audit` 无法运行：registry 指向 npmmirror，不支持 audit 接口，前端依赖
  漏洞面未核实。
- `docs/ROADMAP.md` 自述 `docs/eval-governance.html` 应停止手工维护、改为从
  manifest 生成或删除，尚未处理。

## 最新改进 (2026-08-10)
