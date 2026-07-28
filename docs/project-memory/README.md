# Project memory

写代码前按以下顺序查询：

1. `PROJECT_REGISTRY.yaml`：能力、接口和所有者。
2. `CAPABILITY_MAP.md`：人工维护的架构边界。
3. `MODULE_CATALOG.md` 与 `INTERFACE_CATALOG.md`：生成目录。
4. `SYMBOL_INDEX.md`：详细导航。
5. 只打开命中的源码和附近测试确认行为。

公共模块或符号变化后运行：

```powershell
python scripts/generate_project_memory.py
python scripts/generate_project_memory.py --check
```
