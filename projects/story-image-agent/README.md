# Story Image Agent

独立的故事后续生图项目空间。当前 MVP 只生成可审计的图片生成请求，不伪造
provider 返回的图片；真实 provider 必须由 Rust 的 typed capability 提供。

核心保证：每次提示词修改创建新的 revision；每次生成创建新的 request，旧提示词和
旧图片引用永不覆盖。输入只允许来自已固定的 story package 场景/角色 span。

```powershell
python -m unittest discover -s tests -p "test_*.py"
```
