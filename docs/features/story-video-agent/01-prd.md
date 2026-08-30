# REQ-411..413 — Story Video Agent

Status: G2 contracts ready; provider-neutral MVP implemented

## Requirements

- **REQ-411:** 使用不可变图片 artifact 和至少一个故事/镜头 span 规划视频生成。
- **REQ-412:** 每次生成创建唯一 request，保留图片、故事和镜头参数的关联。
- **REQ-413:** Python 不接收文件路径、密钥或 shell；provider/预算/存储由 Rust owner 执行。

## Acceptance

- 独立 workspace 可单独测试；
- 非 `artifact://sha256/...` 图片输入 fail-closed；
- 缺失故事 span fail-closed；
- typed request 可以由 provider-neutral JSON Schema 验证；
- 多次生成不覆盖旧视频请求或 artifact。

## Exclusions

- 本能力是 image-to-video generation，不等于 deferred 的视频素材提取流水线；
- 当前未选择真实 video provider，不伪造视频结果；
- 不下载平台视频、不绕过版权或反爬机制。
