# 任务 03：真实媒体质量评估

## 目标

把图片/视频质量门禁从手工指标输入推进到可插拔、可审计的质量评估接口，同时保留 fail-closed 和人工复核入口。

## 必须交付

- 明确图片和视频质量指标、范围、阈值及证据来源。
- 区分缺陷：故事对齐、人物一致性、构图/动作、连续性、伪影。
- 输出稳定 schema、失败原因、返工阶段和评估版本。
- 评估失败时不得创建精生成请求。
- 测试缺失指标、越界、NaN、模型异常和版本漂移。

## 写入范围

- `D:/github_dgy/story-image-agent/story_image_agent/quality.py`
- `D:/github_dgy/story-video-agent/story_video_agent/quality.py`
- 对应两个仓库的测试和媒体质量契约。

## 不得修改

不得在 Python 中调用网络、文件系统、shell 或 provider。

## 验收

```powershell
python -m unittest discover -s tests -p "test_*.py"
```
