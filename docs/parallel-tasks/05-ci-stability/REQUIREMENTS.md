# 任务 05：CI 稳定化

## 目标

修复主项目 Rust workspace 和 desktop-windows job 的 CI-only 失败，使每个 job 使用正确 workspace、依赖、venv 和测试条件。

## 必须交付

- 从 GitHub Actions job 日志证明失败根因。
- Rust 测试不依赖并行顺序、开发机路径或共享临时状态。
- desktop-windows 明确在正确目录运行 fmt/clippy/test。
- Windows job 创建并激活规定的 sidecar venv。
- 真实 Tauri acceptance 不因缺少工具链而静默误报；缺失依赖要给出明确诊断。

## 写入范围

- `.github/workflows/`、CI 测试 fixture、Windows acceptance 脚本及必要测试。

## 不得修改

不得删减失败测试或降低断言来“修绿”。

## 验收

以 GitHub Actions 最新 run 的 job/step 结论为最终证据，并在本目录写 `STATUS.md`。
