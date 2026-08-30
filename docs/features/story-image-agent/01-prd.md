# REQ-401..403 — Story Image Agent

Status: G2 contracts ready; provider-neutral MVP implemented

## Requirements

- **REQ-401:** 从已固定的故事场景和角色 span 建立图片提示词，不能接收无来源的生产输入。
- **REQ-402:** 用户修改提示词时创建 append-only revision，保留父 revision 和来源 span。
- **REQ-403:** 每次生成/重新生成创建唯一 request；旧提示词、请求和图片 artifact 不覆盖。

## Acceptance

- 独立 workspace 可单独安装和测试；
- 修改提示词后旧 revision 仍可读取并重新生成；
- 同一 revision 两次生成具有不同 request ID；
- Python 只产出 typed request，不持有 provider 密钥、价格表、存储路径或重试策略；
- 真实图片只接受 Rust capability 返回的 immutable artifact reference。

## Exclusions

- 当前未选择真实 image provider，不调用外部付费接口；
- 不把占位图或测试 fixture 宣称为生成结果；
- 不允许未授权受保护故事或图片进入生产数据。
