# HANDOFF
status: active
date: 2026-08-27
agent: ZCode (GLM-5.3)
branch: feature/eval-p3a-unlock
origin: https://github.com/dgy-github/microcodex-short-drama-studio.git

## 最新改进 (2026-08-27) · 评测体系补完（P1/P3a 解锁线）

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
- ✅ 创建 `IMPROVEMENT_PLAN.md` - 完整的项目完善计划
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
- P1 退出条件失败：seeded_defect_detection = 0.0（目标 0.75）
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
.\.venv\Scripts\Activate.ps1
python -m pip install -e sidecar
python scripts/init_project.py --check
```

全量验证四条缺一不可；桌面端自带独立 workspace：

```powershell
cargo test --workspace --all-features
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
python -m unittest discover -s sidecar -p "test_*.py"
python -m unittest discover -s eval/tools -p "test_*.py"
```

当前产品只写故事，不下载视频、不提取素材、不自动发布。

## 当前结论

- P5-P10 工程实现已完成；P10 clean-Windows Exit 尚未满足。
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
- P1 judge 稳定性仍失败；`seeded_defect_detection = 0.0`，目标 0.75。
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
