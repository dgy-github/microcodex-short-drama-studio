# Handoff：CI 稳定化

## 目标

修复主项目 Rust workspace、desktop-windows 和真实 Tauri acceptance 的 CI-only 失败，确保 job 使用正确 workspace、依赖、venv 和工具诊断。

## 范围

只修改 `.github/workflows/`、CI fixture、Windows acceptance 脚本和必要测试；不得删测试或降低断言。

## 依赖与验收

无前置依赖。必须从 GitHub job 日志证明根因；Windows job 创建规定 venv；以最新 GitHub Actions 全部适用 job 结论为最终证据。

## 状态

`pending`。完成后记录 run URL、失败根因、修复提交和最终 job 结论。
