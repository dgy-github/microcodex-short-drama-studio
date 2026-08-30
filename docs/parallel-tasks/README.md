# 并行任务拆分

这些任务可以在独立 Codex 任务中并行领取。每个任务目录内的 `REQUIREMENTS.md` 是唯一任务说明；任务只修改其中列出的文件范围，避免多个任务互相覆盖。

## 推荐并行分组

| 任务 | 目录 | 写入范围 | 依赖 |
| --- | --- | --- | --- |
| 1. Wan provider | `01-wan-provider` | `story-video-agent` provider adapter 与契约 | 无 |
| 2. Kling provider | `02-kling-provider` | `story-video-agent` provider adapter 与契约 | 无 |
| 3. 质量评估 | `03-media-quality` | 两个 Agent 的质量模块与测试 | 无 |
| 4. clean VM / FFmpeg | `04-clean-vm-media-tools` | 主项目发布脚本、工具清单、验收文档 | 无 |
| 5. CI 稳定化 | `05-ci-stability` | 主项目 CI 与 Windows 验收脚本 | 无 |
| 6. 生图 Agent 完整性 | `06-image-agent-release` | `story-image-agent` README、发布元数据、测试 | 3 |
| 7. 生视频 Agent 完整性 | `07-video-agent-release` | `story-video-agent` README、发布元数据、测试 | 1,2,3 |

任务 1/2/3/4/5 可先同时开始。任务 6/7 等依赖任务合并后再开始，或由同一任务负责整合。

## 领取规则

1. 先阅读对应 `REQUIREMENTS.md`。
2. 只修改该文件列出的写入范围。
3. 完成后运行该文件中的验收命令。
4. 在任务目录的 `STATUS.md` 记录结果、提交号和未决问题。
5. 不修改 `.workbuddy/`、`.zcode/` 或其他任务目录。
