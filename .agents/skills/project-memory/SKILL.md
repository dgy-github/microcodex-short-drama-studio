---
name: project-memory
description: 查询、生成和更新短剧工作室的能力、模块、公共接口与业务规则所有权。用于新增功能、接口、模块或重构责任边界之前，以及公共代码变化之后。
---

# 项目记忆

1. 阅读 `docs/project-memory/README.md` 和 `PROJECT_REGISTRY.yaml`。
2. 依次查询能力地图、模块目录、接口目录，最后查询符号索引。
3. 只打开命中的源码和附近测试。
4. 在计划中记录复用能力、所有者和扩展位置。
5. 相同语义必须扩展现有所有者；新建所有者时说明差异。
6. 公共代码变化后运行：

```powershell
python scripts/generate_project_memory.py
python scripts/generate_project_memory.py --check
```

7. 稳定职责变化时同步更新人工能力注册表。
