# HANDOFF
status: active
date: 2026-08-10
agent: Claude Fable 5
branch: main
origin: https://github.com/dgy-github/microcodex-short-drama-studio.git

## 最新改进 (2026-08-10)

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
