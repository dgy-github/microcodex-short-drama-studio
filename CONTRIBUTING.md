# Contributing

1. 首次开发前运行 `python scripts/init_project.py --check`。
2. 写代码前查询 `docs/project-memory/PROJECT_REGISTRY.yaml`。
3. 跨层功能按 `docs/development/WORKFLOW.md` 的 G0-G7 推进。
4. HTTP 契约先改 `contracts/openapi.yaml`；事件与产物先改 `schemas/`。
5. 使用 `REQ-*`、`API-*`、`BE-*`、`FE-*`、`TEST-*` 建立追踪。
6. 不提交 `.env`、供应商凭据、私有数据或未授权故事文本。
7. 完成前运行初始化检查、项目记忆检查和 AGENTS.md 中的仓库检查。
