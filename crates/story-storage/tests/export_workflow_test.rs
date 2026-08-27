// 集成测试：完整导出工作流程
// 测试从创建包到导出各种格式的完整流程

use serde_json::json;
use std::fs;
use story_storage::{
    package_to_html, package_to_markdown, package_to_plain_text, ExportFormat, ExportOptions,
};

/// 创建一个完整的测试故事包
fn create_full_story_package() -> serde_json::Value {
    json!({
        "schema": "story-package/v1",
        "title": "城市边缘的守护者",
        "premise": "在一个被污染笼罩的未来城市，一名普通清洁工发现自己拥有净化能力",
        "genre": "科幻悬疑",
        "characters": [
            {
                "name": "林晨",
                "description": "28岁清洁工，性格内向但善良",
                "traits": ["责任心强", "观察力敏锐", "不善言辞"]
            },
            {
                "name": "苏雨",
                "description": "环保组织成员，理想主义者",
                "traits": ["热情", "行动力强", "有点冲动"]
            }
        ],
        "episodes": [
            {
                "title": "觉醒",
                "summary": "林晨在清理垃圾时发现自己的特殊能力",
                "scenes": [
                    {
                        "description": "清晨，城市边缘的垃圾处理站",
                        "lines": [
                            {"speaker": "林晨", "text": "又是灰蒙蒙的一天..."},
                            {"action": "林晨触碰到一堆污染物，突然感到一阵暖流"},
                            {"speaker": "林晨", "text": "这是...什么？"}
                        ]
                    },
                    {
                        "description": "垃圾站办公室",
                        "lines": [
                            {"speaker": "苏雨", "text": "你好，我是环保组织的志愿者"},
                            {"speaker": "林晨", "text": "有什么可以帮忙的吗？"}
                        ]
                    }
                ]
            },
            {
                "title": "试探",
                "summary": "林晨尝试理解和控制自己的能力",
                "scenes": [
                    {
                        "description": "夜晚，城市公园",
                        "lines": [
                            {"action": "林晨独自练习使用能力"},
                            {"speaker": "林晨", "text": "如果我能控制它..."}
                        ]
                    }
                ]
            }
        ]
    })
}

#[test]
fn test_full_export_workflow_markdown() {
    let package = create_full_story_package();
    let options = ExportOptions {
        format: ExportFormat::Markdown,
        include_metadata: true,
        include_characters: true,
    };

    let result = package_to_markdown(&package, &options);
    assert!(result.is_ok());

    let markdown = result.unwrap();

    // 验证标题
    assert!(markdown.contains("# 城市边缘的守护者"));

    // 验证元数据
    assert!(markdown.contains("**创意前提**"));
    assert!(markdown.contains("未来城市"));
    assert!(markdown.contains("**类型**: 科幻悬疑"));
    assert!(markdown.contains("**集数**: 2 集"));

    // 验证人物介绍
    assert!(markdown.contains("## 人物介绍"));
    assert!(markdown.contains("### 林晨"));
    assert!(markdown.contains("28岁清洁工"));
    assert!(markdown.contains("责任心强、观察力敏锐、不善言辞"));

    // 验证剧集内容
    assert!(markdown.contains("## 剧集内容"));
    assert!(markdown.contains("### 第 1 集：觉醒"));
    assert!(markdown.contains("**剧情概要**: 林晨在清理垃圾时发现自己的特殊能力"));

    // 验证场景和对话
    assert!(markdown.contains("#### 场景 1"));
    assert!(markdown.contains("*清晨，城市边缘的垃圾处理站*"));
    assert!(markdown.contains("**林晨**: 又是灰蒙蒙的一天..."));
    assert!(markdown.contains("*[林晨触碰到一堆污染物，突然感到一阵暖流]*"));

    // 验证第二集
    assert!(markdown.contains("### 第 2 集：试探"));
}

#[test]
fn test_full_export_workflow_html() {
    let package = create_full_story_package();
    let options = ExportOptions::default();

    let result = package_to_html(&package, &options);
    assert!(result.is_ok());

    let html = result.unwrap();

    // 验证 HTML 结构
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<html lang=\"zh-CN\">"));
    assert!(html.contains("<title>城市边缘的守护者</title>"));

    // 验证 CSS 样式存在
    assert!(html.contains("<style>"));
    assert!(html.contains("body {"));

    // 验证内容
    assert!(html.contains("<h1>城市边缘的守护者</h1>"));
    assert!(html.contains("故事信息"));
    assert!(html.contains("人物介绍"));

    // 验证 HTML 转义
    assert!(!html.contains("<script>"));

    // 验证对话格式
    assert!(html.contains("<span class=\"speaker\">林晨:</span>"));
    assert!(html.contains("又是灰蒙蒙的一天..."));
}

#[test]
fn test_full_export_workflow_plain_text() {
    let package = create_full_story_package();
    let options = ExportOptions {
        format: ExportFormat::PlainText,
        include_metadata: true,
        include_characters: true,
    };

    let result = package_to_plain_text(&package, &options);
    assert!(result.is_ok());

    let text = result.unwrap();

    // 验证纯文本格式
    assert!(text.contains("城市边缘的守护者"));
    assert!(text.contains("创意前提:"));

    // 验证没有标记语言
    assert!(!text.contains("<html>"));
    assert!(!text.contains("##"));

    // 验证人物
    assert!(text.contains("林晨"));
    assert!(text.contains("28岁清洁工"));

    // 验证集数
    assert!(text.contains("第 1 集"));
    assert!(text.contains("第 2 集"));

    // 验证对话格式
    assert!(text.contains("林晨: 又是灰蒙蒙的一天..."));
}

#[test]
fn test_export_all_formats_to_files() {
    let package = create_full_story_package();
    let temp_dir = std::env::temp_dir().join(format!("story-export-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let formats = vec![
        (ExportFormat::Markdown, "story.md"),
        (ExportFormat::Html, "story.html"),
        (ExportFormat::PlainText, "story.txt"),
    ];

    for (format, filename) in formats {
        let options = ExportOptions {
            format,
            include_metadata: true,
            include_characters: true,
        };

        let content = match format {
            ExportFormat::Markdown => package_to_markdown(&package, &options),
            ExportFormat::Html => package_to_html(&package, &options),
            ExportFormat::PlainText => package_to_plain_text(&package, &options),
            ExportFormat::Json => Ok(serde_json::to_string_pretty(&package).unwrap()),
        };

        assert!(content.is_ok());

        let file_path = temp_dir.join(filename);
        fs::write(&file_path, content.unwrap()).unwrap();
        assert!(file_path.exists());

        // 验证文件非空
        let metadata = fs::metadata(&file_path).unwrap();
        assert!(metadata.len() > 0);
    }

    // 清理
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_export_with_minimal_content() {
    // 测试最小化内容的导出（只有标题）
    let minimal_package = json!({
        "title": "最小故事"
    });

    let options = ExportOptions::default();

    // 所有格式都应该能处理
    assert!(package_to_markdown(&minimal_package, &options).is_ok());
    assert!(package_to_html(&minimal_package, &options).is_ok());
    assert!(package_to_plain_text(&minimal_package, &options).is_ok());
}

#[test]
fn test_export_without_optional_sections() {
    let package = create_full_story_package();

    // 不包含元数据
    let no_metadata = ExportOptions {
        format: ExportFormat::Markdown,
        include_metadata: false,
        include_characters: true,
    };

    let result = package_to_markdown(&package, &no_metadata);
    assert!(result.is_ok());
    let markdown = result.unwrap();
    assert!(!markdown.contains("## 故事信息"));
    assert!(!markdown.contains("创意前提"));

    // 不包含人物
    let no_characters = ExportOptions {
        format: ExportFormat::Markdown,
        include_metadata: true,
        include_characters: false,
    };

    let result = package_to_markdown(&package, &no_characters);
    assert!(result.is_ok());
    let markdown = result.unwrap();
    assert!(!markdown.contains("## 人物介绍"));
    // 人物名字仍会出现在对话中，所以我们只检查人物介绍部分不存在
    assert!(!markdown.contains("28岁清洁工"));
    assert!(!markdown.contains("责任心强、观察力敏锐、不善言辞"));

    // 都不包含
    let minimal = ExportOptions {
        format: ExportFormat::Markdown,
        include_metadata: false,
        include_characters: false,
    };

    let result = package_to_markdown(&package, &minimal);
    assert!(result.is_ok());
    let markdown = result.unwrap();
    assert!(markdown.contains("# 城市边缘的守护者")); // 标题仍然存在
    assert!(markdown.contains("## 剧集内容")); // 剧集内容仍然存在
}
