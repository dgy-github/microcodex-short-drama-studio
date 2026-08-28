# Troubleshooting Guide

常见问题和解决方案。

---

## 🔧 环境和安装问题

### Python 依赖安装失败

**症状**: `ModuleNotFoundError: No module named 'campaign'`

**原因**: `campaign-muti-agent` 依赖未安装

**解决方案**:
```powershell
# 创建并激活虚拟环境
python -m venv .venv

# 安装 sidecar（包含 campaign 依赖）
.\.venv\Scripts\python.exe -m pip install -e sidecar
.\.venv\Scripts\python.exe -m pip install -r scripts/requirements.txt
.\.venv\Scripts\python.exe -m pip install -r eval/tools/requirements.txt
```

### Rust 编译失败

**症状**: `error: linking with 'link.exe' failed`

**解决方案**:
1. 确认 Rust 版本是 1.88.0: `rustc --version`
2. 安装 Visual Studio Build Tools 2019+
3. 重新运行: `cargo build --workspace`

### Node.js 版本不匹配

**症状**: 桌面应用构建失败

**解决方案**:
```bash
# 使用 nvm 切换到正确版本
nvm install 22.14.0
nvm use 22.14.0
```

---

## 🧪 测试问题

### 测试失败: "working tree clean"

**症状**: `cargo test` 报告 git 状态不干净

**原因**: 测试验证仓库状态

**解决方案**:
```bash
# 提交或暂存更改
git add .
git commit -m "your message"

# 或者暂存
git stash
```

### Python 测试输出有 RETRY 消息

**症状**: 看到 `RETRY HTTP 429` 或 `RETRY RemoteDisconnected`

**说明**: 这些是正常的重试日志（已移到 stderr），不是错误。如果测试通过，说明重试成功。

**验证**:
```powershell
.\.venv\Scripts\python.exe -m unittest discover -s eval/tools -p "test_*.py" 2>&1 | Select-String "OK|FAILED"
```

### 桌面端测试找不到

**症状**: `cargo test --workspace` 没有运行桌面测试

**原因**: 桌面端在独立的 workspace

**解决方案**:
```powershell
# 桌面端需要单独测试
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

---

## 🔐 凭据和配置问题

### Windows Credential Manager 访问失败

**症状**: `keyring error: no backend available`

**解决方案**:
1. 确认在 Windows 10+ 系统上运行
2. 检查 Credential Manager 服务是否运行:
   ```powershell
   Get-Service -Name "VaultSvc" | Select-Object Status
   ```
3. 如果服务未运行: `Start-Service -Name "VaultSvc"`

### Provider 凭据未配置

**症状**: 运行时提示 "missing provider credentials"

**解决方案**:
1. 打开桌面应用
2. 进入设置 → Provider Configuration
3. 配置 DeepSeek 和百炼的 API keys
4. 运行健康检查

---

## 🚀 运行时问题

### Story 生成失败: token_budget_exceeded

**症状**: 运行在 t15 或类似任务失败

**原因**: Token 预算不足

**解决方案**:
1. 增加 budget.max_tokens（推荐 180,000+）
2. 减少集数（8 → 6）
3. 使用短集数约束配置

### Sidecar 启动超时

**症状**: `SidecarProcessError: launch timeout`

**解决方案**:
1. 检查 Python 环境:
   ```powershell
   .\.venv\Scripts\python.exe -c "import campaign; print('OK')"
   ```
2. 检查端口占用:
   ```powershell
   netstat -ano | findstr "127.0.0.1:8700"
   ```
3. 增加超时时间（在代码中调整 SidecarLaunchConfig）

### Event replay 失败

**症状**: 重启后无法恢复任务

**原因**: Event log 损坏或 Last-Event-ID 不匹配

**解决方案**:
1. 检查 `artifacts/advisory-runs/<run_id>/events.jsonl`
2. 验证每行都是有效 JSON
3. 如果损坏，使用备份恢复（如果有）
4. 最坏情况：删除 run 目录重新开始

---

## 📦 打包和发布问题

### MSI/NSIS 构建失败

**症状**: `tauri build` 失败

**解决方案**:
1. 确认所有依赖已安装:
   ```powershell
   # 检查 PyInstaller sidecar
   python scripts/build_sidecar.py --check
   ```
2. 清理并重建:
   ```powershell
   cargo clean
   cd apps/desktop
   npm run tauri build
   ```

### 安装包 SmartScreen 警告

**症状**: Windows 显示"未识别的应用"

**说明**: 这是正常的，因为使用未签名的安装包

**解决方案**:
1. 点击"更多信息"
2. 点击"仍要运行"
3. 或者配置 Authenticode 签名（见 P10 文档）

---

## 🐛 已知问题和限制

### Judge 稳定性问题

**状态**: P1 阻塞项

**问题**: `seeded_defect_detection = 0.0`（目标 0.75）

**临时方案**: 
- 所有输出标记为 `advisory/non-promotable`
- 等待人工盲测完成

**跟踪**: docs/ROADMAP.md P1

### GLM 路由欠费

**状态**: 外部依赖问题

**问题**: 智谱直连和火山 Ark 均显示余额不足

**临时方案**: 
- 当前使用 qwen + openai 两个 judge 族
- 充值后可恢复第三个 judge 族

**跟踪**: docs/reviews/2026-07-29-audit.md #8

### 桌面端测试覆盖较低

**状态**: 技术债务

**问题**: ~4067 行代码只有 19 个测试

**计划**: 阶段 2.2 - 提升测试覆盖

**跟踪**: HANDOFF.md

---

## 📞 获取帮助

### 查看日志

```powershell
# Rust 日志
$env:RUST_LOG="debug"
cargo run

# Python 日志
python -m sidecar.story_sidecar --verbose

# 桌面应用日志
# Windows: %APPDATA%\microcodex-short-drama-studio\logs\
```

### 诊断工具

```powershell
# 项目完整性检查
.\.venv\Scripts\python.exe scripts/init_project.py --check

# Provider 连通性检查
python eval/tools/run_stage0_probe.py --check-connectivity

# 存储完整性检查
cargo test --package story-storage

# 系统环境检查
rustc --version
python --version
node --version
git --version
```

### 报告问题

提交 issue 时请包含：

1. **环境信息**:
   - OS 版本: `systeminfo | findstr /B /C:"OS Name" /C:"OS Version"`
   - Rust: `rustc --version`
   - Python: `python --version`
   - Node: `node --version`

2. **重现步骤**: 详细的操作步骤

3. **错误信息**: 完整的错误输出

4. **相关日志**: 
   - Rust backtrace: `$env:RUST_BACKTRACE=1`
   - Python traceback: 完整 stack trace

5. **预期行为**: 你期望发生什么

---

## 🔍 调试技巧

### 启用详细日志

```powershell
# Rust
$env:RUST_LOG="trace"
$env:RUST_BACKTRACE="full"

# Python
import logging
logging.basicConfig(level=logging.DEBUG)
```

### 单步调试

**Rust**:
```bash
# 使用 rust-lldb 或 rust-gdb
cargo build
rust-lldb target/debug/your-binary
```

**Python**:
```python
# 使用 pdb
import pdb; pdb.set_trace()
```

### 性能分析

```powershell
# Rust - 使用 flamegraph
cargo install flamegraph
cargo flamegraph --bin your-binary

# Python - 使用 cProfile
python -m cProfile -o profile.stats your_script.py
python -m pstats profile.stats
```

---

## 💡 最佳实践

### 开发前

1. ✅ 拉取最新代码: `git pull`
2. ✅ 激活虚拟环境: `.\.venv\Scripts\Activate.ps1`
3. ✅ 运行完整测试确认基线正常
4. ✅ 查看 HANDOFF.md 了解当前状态

### 提交前

1. ✅ 运行所有测试
2. ✅ 运行格式化: `cargo fmt --all`
3. ✅ 运行 linter: `cargo clippy`
4. ✅ 检查项目完整性: `.\.venv\Scripts\python.exe scripts/init_project.py --check`
5. ✅ 更新相关文档

### 遇到问题时

1. ✅ 先查看本文档
2. ✅ 搜索 issues
3. ✅ 检查 docs/reviews/2026-07-29-audit.md 的已知问题
4. ✅ 启用详细日志再重现
5. ✅ 如果是新问题，详细记录并报告

---

更新时间: 2026-08-10
维护者: 项目所有者
