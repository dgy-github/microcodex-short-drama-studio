// 集成测试：完整导出工作流程
// 使用版本控制中的真实 baseline story-package 数据，验证导出内容与 story-package/v1 schema 对齐

use std::fs;
use story_storage::{
    package_to_html, package_to_markdown, package_to_plain_text, ExportFormat, ExportOptions,
};

/// 读取真实 baseline 故事包（family_001）
fn real_package() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../eval/baselines/baseline-deepseek-v4-pro-20260727/family_001.story-package.json"
    ))
    .unwrap()
}

#[test]
fn test_full_export_workflow_markdown() {
    let package = real_package();
    let options = ExportOptions {
        include_metadata: true,
        include_characters: true,
    };

    let markdown = package_to_markdown(&package, &options).unwrap();

    // 标题来自 logline.text
    assert!(markdown.contains("# 母亲卖掉老房子"));

    // 元数据来自 promise
    assert!(markdown.contains("**类型**: family"));
    assert!(markdown.contains("**集数**: 8 集"));

    // 人物介绍来自 characters 真实字段
    assert!(markdown.contains("## 人物介绍"));
    assert!(markdown.contains("### 林母"));
    assert!(markdown.contains("希望子女常回家看看，家庭和睦"));

    // 情节节拍
    assert!(markdown.contains("## 情节节拍"));
    assert!(markdown.contains("母亲突然通知卖房，子女不知所措"));

    // 剧集内容来自 opening_state 等真实字段
    assert!(markdown.contains("## 剧集内容"));
    assert!(markdown.contains("### 第 1 集"));
    assert!(markdown.contains("林母电话告知卖房，子女震惊"));

    // 场景（顶层）与台词，speaker 解析为角色名
    assert!(markdown.contains("## 场景"));
    assert!(markdown.contains("老房子客厅"));
    assert!(markdown.contains("**林母**：尝尝这红烧肉"));
    assert!(markdown.contains("餐桌上摆满家常菜"));
}

#[test]
fn test_full_export_workflow_html() {
    let package = real_package();
    let html = package_to_html(&package, &ExportOptions::default()).unwrap();

    // 验证 HTML 结构
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<html lang=\"zh-CN\">"));
    assert!(html.contains("<title>母亲卖掉老房子"));

    // 验证 CSS 样式存在
    assert!(html.contains("<style>"));
    assert!(html.contains("body {"));

    // 验证内容
    assert!(html.contains("<h1>母亲卖掉老房子"));
    assert!(html.contains("故事信息"));
    assert!(html.contains("人物介绍"));
    assert!(html.contains("林母"));

    // 验证对话格式与 speaker 解析
    assert!(html.contains("<span class=\"speaker\">林母:</span>"));
    assert!(html.contains("尝尝这红烧肉"));
}

#[test]
fn test_full_export_workflow_plain_text() {
    let package = real_package();
    let options = ExportOptions {
        include_metadata: true,
        include_characters: true,
    };

    let text = package_to_plain_text(&package, &options).unwrap();

    assert!(text.contains("母亲卖掉老房子"));
    assert!(text.contains("类型: family"));

    // 验证没有标记语言
    assert!(!text.contains("<html>"));
    assert!(!text.contains("##"));
    assert!(!text.contains("**"));

    // 验证人物
    assert!(text.contains("人物介绍"));
    assert!(text.contains("林母"));

    // 验证集数与场景
    assert!(text.contains("第 1 集"));
    assert!(text.contains("场景 1"));

    // 验证对话格式与 speaker 解析
    assert!(text.contains("林母: 尝尝这红烧肉"));
    assert!(text.contains("餐桌上摆满家常菜"));
}

#[test]
fn test_export_all_formats_to_files() {
    let package = real_package();
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
            include_metadata: true,
            include_characters: true,
        };

        let content = match format {
            ExportFormat::Markdown => package_to_markdown(&package, &options),
            ExportFormat::Html => package_to_html(&package, &options),
            ExportFormat::PlainText => package_to_plain_text(&package, &options),
            ExportFormat::Json => Ok(serde_json::to_string_pretty(&package).unwrap()),
        };

        let content = content.unwrap();
        let file_path = temp_dir.join(filename);
        fs::write(&file_path, &content).unwrap();
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
    // 空包必须能被所有格式安全处理
    let minimal_package = serde_json::json!({});

    let options = ExportOptions::default();

    assert!(package_to_markdown(&minimal_package, &options).is_ok());
    assert!(package_to_html(&minimal_package, &options).is_ok());
    assert!(package_to_plain_text(&minimal_package, &options).is_ok());
}

#[test]
fn test_export_without_optional_sections() {
    let package = real_package();

    // 不包含元数据
    let no_metadata = ExportOptions {
        include_metadata: false,
        include_characters: true,
    };
    let markdown = package_to_markdown(&package, &no_metadata).unwrap();
    assert!(!markdown.contains("## 故事信息"));
    assert!(!markdown.contains("**类型**"));
    // 正文仍保留
    assert!(markdown.contains("## 人物介绍"));
    assert!(markdown.contains("## 剧集内容"));

    // 不包含人物
    let no_characters = ExportOptions {
        include_metadata: true,
        include_characters: false,
    };
    let markdown = package_to_markdown(&package, &no_characters).unwrap();
    assert!(!markdown.contains("## 人物介绍"));
    // 对话中的角色名仍会通过 speaker 解析出现
    assert!(markdown.contains("林母"));

    // 都不包含，标题与正文仍在
    let minimal = ExportOptions {
        include_metadata: false,
        include_characters: false,
    };
    let markdown = package_to_markdown(&package, &minimal).unwrap();
    assert!(markdown.contains("# 母亲卖掉老房子"));
    assert!(markdown.contains("## 剧集内容"));
    assert!(markdown.contains("## 场景"));
}

#[test]
fn test_export_format_from_extension() {
    assert_eq!(
        ExportFormat::from_extension("json"),
        Some(ExportFormat::Json)
    );
    assert_eq!(
        ExportFormat::from_extension("md"),
        Some(ExportFormat::Markdown)
    );
    assert_eq!(
        ExportFormat::from_extension("html"),
        Some(ExportFormat::Html)
    );
    assert_eq!(
        ExportFormat::from_extension("txt"),
        Some(ExportFormat::PlainText)
    );
    assert_eq!(ExportFormat::from_extension("xyz"), None);
}
