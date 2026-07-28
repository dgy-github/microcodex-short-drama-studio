---
name: project-init
description: 初始化并验证短剧工作室项目必须具备的治理、项目记忆、接口契约、需求追踪、测试和 CI 基础。用于首次开发、仓库复制或移动、初始化检查失败时。
---

# 项目初始化

本技能是所有开发工作的前置技能。

1. 完整阅读 `AGENTS.md`、`.project/init.yaml`、开发流程和项目记忆说明。
2. 运行 `python scripts/init_project.py --check`。
3. 若尚未初始化或目录位置改变，运行：

```powershell
python scripts/init_project.py --name "MicrocodeX Short Drama Studio"
```

4. 不自动安装依赖；依赖安装必须单独确认。
5. 检查项目名称、CODEOWNERS、OpenAPI、能力注册表和示例配置。
6. 再次运行 `--check`，报告结果和跳过项。

不得删除治理文件绕过初始化，不得覆盖真实产品契约。
