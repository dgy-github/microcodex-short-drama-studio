# Handoff：FFmpeg 与 clean VM 媒体验收

## 目标

提供固定版本、SHA-256、许可证证据的 FFmpeg 工具链，并在干净 Windows VM 完成真实裁剪、补段、拼接和产物校验。

## 范围

只修改主项目发布脚本、工具 manifest、Windows 验收脚本/文档及相关测试；不得提交未核实许可证的二进制或绕过 hash 校验。

## 依赖与验收

无前置依赖。clean VM 不得依赖开发机 PATH、缓存、venv 或特殊用户目录；验证真实 MP4、取消、超时、partial 清理和 artifact hash。

## 状态

`pending`。完成后记录工具来源、版本/hash、许可证、VM 镜像、测试结果和提交号。
