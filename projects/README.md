# 独立 Agent 项目

生图和视频 Agent 已迁移为主仓库同级的独立 Git 项目：

- `../story-image-agent`：故事驱动的提示词 revision 与图片生成请求规划
- `../story-video-agent`：图片 artifact + 故事 span 驱动的视频请求规划

两个项目不再把 Python 源码放在本仓库中。Rust 契约、可信存储、provider、执行和
Desktop 集成仍由本仓库拥有；Agent 通过已审查的 JSON 契约与这些能力协作。
