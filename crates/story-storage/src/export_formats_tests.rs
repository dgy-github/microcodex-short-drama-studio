use super::*;
use serde_json::json;

fn real_package() -> Value {
    serde_json::from_str(include_str!(
        "../../../eval/baselines/baseline-deepseek-v4-pro-20260727/family_001.story-package.json"
    ))
    .unwrap()
}

#[test]
fn test_format_from_extension() {
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

#[test]
fn test_export_format_extension_roundtrip() {
    for format in [
        ExportFormat::Json,
        ExportFormat::Markdown,
        ExportFormat::Html,
        ExportFormat::PlainText,
    ] {
        assert_eq!(
            ExportFormat::from_extension(format.extension()),
            Some(format)
        );
    }
}

#[test]
fn markdown_exports_real_package_fields() {
    let markdown = package_to_markdown(&real_package(), &ExportOptions::default()).unwrap();
    for expected in [
        "# 母亲卖掉老房子",
        "**类型**: family",
        "**集数**: 8 集",
        "## 人物介绍",
        "### 林母",
        "希望子女常回家看看，家庭和睦",
        "老房子其实早已破旧难修",
        "## 情节节拍",
        "母亲突然通知卖房，子女不知所措",
        "## 剧集内容",
        "### 第 1 集",
        "林母电话告知卖房，子女震惊",
        "**结尾钩子**",
        "## 场景",
        "老房子客厅",
        "**林母**：尝尝这红烧肉",
        "潜台词",
        "餐桌上摆满家常菜",
    ] {
        assert!(markdown.contains(expected), "missing {expected}");
    }
}

#[test]
fn plain_text_exports_real_package_fields() {
    let text = package_to_plain_text(&real_package(), &ExportOptions::default()).unwrap();
    for expected in [
        "母亲卖掉老房子",
        "类型: family",
        "人物介绍",
        "林母",
        "第 1 集",
        "林母: 尝尝这红烧肉",
        "餐桌上摆满家常菜",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    for marker in ["##", "<h", "**"] {
        assert!(!text.contains(marker));
    }
}

#[test]
fn html_exports_real_package_fields_and_escapes() {
    let html = package_to_html(&real_package(), &ExportOptions::default()).unwrap();
    for expected in [
        "<!DOCTYPE html>",
        "<html lang=\"zh-CN\">",
        "<title>母亲卖掉老房子",
        "<h1>母亲卖掉老房子",
        "故事信息",
        "人物介绍",
        "林母",
        "<span class=\"speaker\">林母:</span>",
        "尝尝这红烧肉",
    ] {
        assert!(html.contains(expected), "missing {expected}");
    }
}

#[test]
fn html_escape_neutralizes_script_and_entities() {
    assert_eq!(html_escape("hello"), "hello");
    assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    assert_eq!(html_escape("A & B"), "A &amp; B");
}

#[test]
fn exports_are_empty_safe() {
    let package = json!({});
    let options = ExportOptions::default();
    assert!(package_to_markdown(&package, &options).is_ok());
    assert!(package_to_html(&package, &options).is_ok());
    assert!(package_to_plain_text(&package, &options).is_ok());
}

#[test]
fn metadata_can_be_disabled() {
    let options = ExportOptions {
        include_metadata: false,
        include_characters: true,
    };
    let markdown = package_to_markdown(&real_package(), &options).unwrap();
    assert!(!markdown.contains("## 故事信息"));
    assert!(!markdown.contains("**类型**"));
    assert!(markdown.contains("## 人物介绍"));
    assert!(markdown.contains("## 剧集内容"));
}

#[test]
fn characters_can_be_disabled() {
    let options = ExportOptions {
        include_metadata: true,
        include_characters: false,
    };
    let markdown = package_to_markdown(&real_package(), &options).unwrap();
    assert!(!markdown.contains("## 人物介绍"));
    assert!(markdown.contains("林母"));
}

#[test]
fn speaker_span_resolves_to_name() {
    let names = character_names(&real_package());
    assert_eq!(resolve_speaker(&names, "story-package/char-1"), "林母");
    assert_eq!(resolve_speaker(&names, "char-2"), "林建国");
    assert_eq!(
        resolve_speaker(&names, "story-package/char-99"),
        "story-package/char-99"
    );
}

#[test]
fn episode_ref_resolves_to_label() {
    assert_eq!(episode_label("story-package/ep-3"), "第 3 集");
    assert_eq!(episode_label("story-package/ep-12"), "第 12 集");
}
