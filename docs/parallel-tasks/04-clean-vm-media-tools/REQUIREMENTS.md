# 任务 04：FFmpeg 与 clean VM 媒体验收

## 目标

提供固定版本、hash、许可证证据的 FFmpeg 发布资源，并在干净 Windows VM/提取目录完成真实裁剪、补段、拼接验收。

## 必须交付

- 固定 FFmpeg 版本、下载来源、SHA-256 和许可证文件。
- 发布脚本将工具写入可信 `tools` 目录并生成 manifest。
- clean VM 不依赖开发机 PATH、缓存、venv 或用户目录特殊权限。
- 真实 MP4 输入完成 trim/concat，输出经过 MIME、长度和 hash 校验。
- 失败、超时和取消留下可审计证据且无 partial 文件。

## 写入范围

- 主项目 `scripts/`、发布配置、工具 manifest、Windows 验收文档及相关测试。

## 不得修改

不得绕过 manifest hash 校验，不得提交未核实许可证的二进制。

## 验收

运行项目规定的 Rust、Windows 发布 smoke 和真实媒体验收命令，并在本目录写 `STATUS.md`。
