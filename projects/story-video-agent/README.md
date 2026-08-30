# Story Video Agent

独立的故事视频 Agent 项目空间。MVP 将不可变图片 artifact 与故事/镜头 span 组合成
可审计的视频生成请求；真实视频 provider、费用、存储、重试、取消和事件投递由共享的
Rust trusted capability / story-runtime 承担。

图片必须是 `artifact://sha256/<digest>` 引用，故事输入必须有至少一个稳定 span；不接收
本地路径，不覆盖已有视频产物。

```powershell
python -m unittest discover -s tests -p "test_*.py"
```
