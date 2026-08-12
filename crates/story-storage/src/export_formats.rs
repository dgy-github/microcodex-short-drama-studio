// Export functionality enhancement for story packages
// Supports multiple export formats: JSON, Markdown, HTML, TXT

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Markdown,
    Html,
    PlainText,
}

impl ExportFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "md" | "markdown" => Some(Self::Markdown),
            "html" | "htm" => Some(Self::Html),
            "txt" => Some(Self::PlainText),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
            Self::Html => "html",
            Self::PlainText => "txt",
        }
    }
}

pub struct ExportOptions {
    pub format: ExportFormat,
    pub include_metadata: bool,
    pub include_characters: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Json,
            include_metadata: true,
            include_characters: true,
        }
    }
}

/// Convert story package to Markdown format
pub fn package_to_markdown(package: &Value, options: &ExportOptions) -> Result<String, String> {
    let mut output = String::new();

    // Title
    if let Some(title) = package.get("title").and_then(Value::as_str) {
        output.push_str(&format!("# {}\n\n", title));
    }

    // Metadata section
    if options.include_metadata {
        output.push_str("## 故事信息\n\n");

        if let Some(premise) = package.get("premise").and_then(Value::as_str) {
            output.push_str(&format!("**创意前提**: {}\n\n", premise));
        }

        if let Some(genre) = package.get("genre").and_then(Value::as_str) {
            output.push_str(&format!("**类型**: {}\n\n", genre));
        }

        if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
            output.push_str(&format!("**集数**: {} 集\n\n", episodes.len()));
        }

        output.push_str("---\n\n");
    }

    // Characters section
    if options.include_characters {
        if let Some(characters) = package.get("characters").and_then(Value::as_array) {
            if !characters.is_empty() {
                output.push_str("## 人物介绍\n\n");

                for character in characters {
                    if let Some(name) = character.get("name").and_then(Value::as_str) {
                        output.push_str(&format!("### {}\n\n", name));

                        if let Some(description) = character.get("description").and_then(Value::as_str) {
                            output.push_str(&format!("{}\n\n", description));
                        }

                        if let Some(traits) = character.get("traits").and_then(Value::as_array) {
                            output.push_str("**特点**: ");
                            let trait_list: Vec<String> = traits
                                .iter()
                                .filter_map(Value::as_str)
                                .map(String::from)
                                .collect();
                            output.push_str(&trait_list.join("、"));
                            output.push_str("\n\n");
                        }
                    }
                }

                output.push_str("---\n\n");
            }
        }
    }

    // Episodes section
    if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
        output.push_str("## 剧集内容\n\n");

        for (index, episode) in episodes.iter().enumerate() {
            let episode_number = index + 1;

            // Episode title
            let default_title = format!("第 {} 集", episode_number);
            let episode_title = episode
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(&default_title);

            output.push_str(&format!("### 第 {} 集：{}\n\n", episode_number, episode_title));

            // Episode summary
            if let Some(summary) = episode.get("summary").and_then(Value::as_str) {
                output.push_str(&format!("**剧情概要**: {}\n\n", summary));
            }

            // Scenes
            if let Some(scenes) = episode.get("scenes").and_then(Value::as_array) {
                for (scene_index, scene) in scenes.iter().enumerate() {
                    output.push_str(&format!("#### 场景 {}\n\n", scene_index + 1));

                    // Scene description
                    if let Some(description) = scene.get("description").and_then(Value::as_str) {
                        output.push_str(&format!("*{}*\n\n", description));
                    }

                    // Dialogue
                    if let Some(lines) = scene.get("lines").and_then(Value::as_array) {
                        for line in lines {
                            if let Some(speaker) = line.get("speaker").and_then(Value::as_str) {
                                if let Some(text) = line.get("text").and_then(Value::as_str) {
                                    output.push_str(&format!("**{}**: {}\n\n", speaker, text));
                                }
                            } else if let Some(action) = line.get("action").and_then(Value::as_str) {
                                output.push_str(&format!("*[{}]*\n\n", action));
                            }
                        }
                    }

                    output.push_str("\n");
                }
            }

            output.push_str("---\n\n");
        }
    }

    // Ending
    output.push_str("---\n\n*故事完*\n");

    Ok(output)
}

/// Convert story package to plain text format
pub fn package_to_plain_text(package: &Value, options: &ExportOptions) -> Result<String, String> {
    let mut output = String::new();

    // Title
    if let Some(title) = package.get("title").and_then(Value::as_str) {
        output.push_str(&format!("{}\n", title));
        output.push_str(&"=".repeat(title.chars().count()));
        output.push_str("\n\n");
    }

    // Metadata
    if options.include_metadata {
        if let Some(premise) = package.get("premise").and_then(Value::as_str) {
            output.push_str(&format!("创意前提: {}\n\n", premise));
        }
    }

    // Characters
    if options.include_characters {
        if let Some(characters) = package.get("characters").and_then(Value::as_array) {
            if !characters.is_empty() {
                output.push_str("人物介绍\n");
                output.push_str("---------\n\n");

                for character in characters {
                    if let Some(name) = character.get("name").and_then(Value::as_str) {
                        output.push_str(&format!("{}\n", name));

                        if let Some(description) = character.get("description").and_then(Value::as_str) {
                            output.push_str(&format!("  {}\n\n", description));
                        }
                    }
                }

                output.push_str("\n");
            }
        }
    }

    // Episodes
    if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
        for (index, episode) in episodes.iter().enumerate() {
            output.push_str(&format!("\n第 {} 集\n", index + 1));
            output.push_str(&"-".repeat(20));
            output.push_str("\n\n");

            if let Some(scenes) = episode.get("scenes").and_then(Value::as_array) {
                for scene in scenes {
                    if let Some(description) = scene.get("description").and_then(Value::as_str) {
                        output.push_str(&format!("[{}]\n\n", description));
                    }

                    if let Some(lines) = scene.get("lines").and_then(Value::as_array) {
                        for line in lines {
                            if let Some(speaker) = line.get("speaker").and_then(Value::as_str) {
                                if let Some(text) = line.get("text").and_then(Value::as_str) {
                                    output.push_str(&format!("{}: {}\n", speaker, text));
                                }
                            } else if let Some(action) = line.get("action").and_then(Value::as_str) {
                                output.push_str(&format!("[{}]\n", action));
                            }
                        }
                    }

                    output.push_str("\n");
                }
            }
        }
    }

    output.push_str("\n--- 完 ---\n");

    Ok(output)
}

/// Convert story package to HTML format
pub fn package_to_html(package: &Value, options: &ExportOptions) -> Result<String, String> {
    let mut output = String::new();

    // HTML header
    output.push_str("<!DOCTYPE html>\n");
    output.push_str("<html lang=\"zh-CN\">\n");
    output.push_str("<head>\n");
    output.push_str("  <meta charset=\"UTF-8\">\n");
    output.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");

    if let Some(title) = package.get("title").and_then(Value::as_str) {
        output.push_str(&format!("  <title>{}</title>\n", html_escape(title)));
    }

    // CSS styles
    output.push_str("  <style>\n");
    output.push_str("    body { font-family: 'Microsoft YaHei', sans-serif; line-height: 1.8; max-width: 800px; margin: 0 auto; padding: 20px; }\n");
    output.push_str("    h1 { color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 10px; }\n");
    output.push_str("    h2 { color: #34495e; margin-top: 40px; }\n");
    output.push_str("    h3 { color: #7f8c8d; }\n");
    output.push_str("    .metadata { background: #ecf0f1; padding: 15px; border-radius: 5px; margin: 20px 0; }\n");
    output.push_str("    .character { margin: 20px 0; padding: 15px; border-left: 4px solid #3498db; background: #f8f9fa; }\n");
    output.push_str("    .episode { margin: 30px 0; }\n");
    output.push_str("    .scene { margin: 20px 0; padding: 15px; background: #fafafa; border-radius: 5px; }\n");
    output.push_str("    .dialogue { margin: 10px 0; }\n");
    output.push_str("    .speaker { font-weight: bold; color: #2980b9; }\n");
    output.push_str("    .action { font-style: italic; color: #7f8c8d; }\n");
    output.push_str("  </style>\n");
    output.push_str("</head>\n");
    output.push_str("<body>\n");

    // Title
    if let Some(title) = package.get("title").and_then(Value::as_str) {
        output.push_str(&format!("  <h1>{}</h1>\n", html_escape(title)));
    }

    // Metadata
    if options.include_metadata {
        output.push_str("  <div class=\"metadata\">\n");
        output.push_str("    <h2>故事信息</h2>\n");

        if let Some(premise) = package.get("premise").and_then(Value::as_str) {
            output.push_str(&format!("    <p><strong>创意前提</strong>: {}</p>\n", html_escape(premise)));
        }

        output.push_str("  </div>\n");
    }

    // Characters
    if options.include_characters {
        if let Some(characters) = package.get("characters").and_then(Value::as_array) {
            if !characters.is_empty() {
                output.push_str("  <h2>人物介绍</h2>\n");

                for character in characters {
                    if let Some(name) = character.get("name").and_then(Value::as_str) {
                        output.push_str("  <div class=\"character\">\n");
                        output.push_str(&format!("    <h3>{}</h3>\n", html_escape(name)));

                        if let Some(description) = character.get("description").and_then(Value::as_str) {
                            output.push_str(&format!("    <p>{}</p>\n", html_escape(description)));
                        }

                        output.push_str("  </div>\n");
                    }
                }
            }
        }
    }

    // Episodes
    if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
        output.push_str("  <h2>剧集内容</h2>\n");

        for (index, episode) in episodes.iter().enumerate() {
            output.push_str("  <div class=\"episode\">\n");
            output.push_str(&format!("    <h3>第 {} 集</h3>\n", index + 1));

            if let Some(scenes) = episode.get("scenes").and_then(Value::as_array) {
                for scene in scenes {
                    output.push_str("    <div class=\"scene\">\n");

                    if let Some(description) = scene.get("description").and_then(Value::as_str) {
                        output.push_str(&format!("      <p class=\"action\">[{}]</p>\n", html_escape(description)));
                    }

                    if let Some(lines) = scene.get("lines").and_then(Value::as_array) {
                        for line in lines {
                            if let Some(speaker) = line.get("speaker").and_then(Value::as_str) {
                                if let Some(text) = line.get("text").and_then(Value::as_str) {
                                    output.push_str("      <div class=\"dialogue\">\n");
                                    output.push_str(&format!("        <span class=\"speaker\">{}:</span> {}\n",
                                        html_escape(speaker), html_escape(text)));
                                    output.push_str("      </div>\n");
                                }
                            } else if let Some(action) = line.get("action").and_then(Value::as_str) {
                                output.push_str(&format!("      <p class=\"action\">[{}]</p>\n", html_escape(action)));
                            }
                        }
                    }

                    output.push_str("    </div>\n");
                }
            }

            output.push_str("  </div>\n");
        }
    }

    output.push_str("</body>\n");
    output.push_str("</html>\n");

    Ok(output)
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(ExportFormat::from_extension("json"), Some(ExportFormat::Json));
        assert_eq!(ExportFormat::from_extension("md"), Some(ExportFormat::Markdown));
        assert_eq!(ExportFormat::from_extension("html"), Some(ExportFormat::Html));
        assert_eq!(ExportFormat::from_extension("txt"), Some(ExportFormat::PlainText));
        assert_eq!(ExportFormat::from_extension("xyz"), None);
    }

    #[test]
    fn test_markdown_export_basic() {
        let package = json!({
            "title": "测试故事",
            "premise": "一个测试的前提",
            "episodes": [
                {
                    "title": "开始",
                    "scenes": [
                        {
                            "description": "室内场景",
                            "lines": [
                                {"speaker": "角色A", "text": "你好"},
                                {"action": "角色A站起来"}
                            ]
                        }
                    ]
                }
            ]
        });

        let options = ExportOptions::default();
        let result = package_to_markdown(&package, &options);

        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.contains("# 测试故事"));
        assert!(markdown.contains("**角色A**: 你好"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("hello"), "hello");
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn test_markdown_export_with_characters() {
        let package = json!({
            "title": "角色测试",
            "characters": [
                {
                    "name": "主角",
                    "description": "勇敢的战士",
                    "traits": ["勇敢", "善良", "坚强"]
                }
            ],
            "episodes": []
        });

        let options = ExportOptions {
            format: ExportFormat::Markdown,
            include_metadata: true,
            include_characters: true,
        };

        let result = package_to_markdown(&package, &options);
        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.contains("## 人物介绍"));
        assert!(markdown.contains("### 主角"));
        assert!(markdown.contains("勇敢的战士"));
        assert!(markdown.contains("勇敢、善良、坚强"));
    }

    #[test]
    fn test_markdown_export_without_metadata() {
        let package = json!({
            "title": "简单故事",
            "premise": "这个不应该出现",
            "episodes": []
        });

        let options = ExportOptions {
            format: ExportFormat::Markdown,
            include_metadata: false,
            include_characters: true,
        };

        let result = package_to_markdown(&package, &options);
        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.contains("# 简单故事"));
        assert!(!markdown.contains("故事信息"));
        assert!(!markdown.contains("这个不应该出现"));
    }

    #[test]
    fn test_html_export_basic_structure() {
        let package = json!({
            "title": "HTML测试",
            "episodes": [
                {"title": "第一集", "scenes": []}
            ]
        });

        let options = ExportOptions::default();
        let result = package_to_html(&package, &options);

        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>HTML测试</title>"));
        assert!(html.contains("<h1>HTML测试</h1>"));
        assert!(html.contains("<h3>第 1 集</h3>"));
    }

    #[test]
    fn test_html_export_xss_protection() {
        let package = json!({
            "title": "<script>alert('xss')</script>",
            "episodes": []
        });

        let options = ExportOptions::default();
        let result = package_to_html(&package, &options);

        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>alert"));
    }

    #[test]
    fn test_plain_text_export() {
        let package = json!({
            "title": "纯文本测试",
            "episodes": [
                {
                    "title": "开场",
                    "scenes": [
                        {
                            "lines": [
                                {"speaker": "A", "text": "你好"}
                            ]
                        }
                    ]
                }
            ]
        });

        let options = ExportOptions {
            format: ExportFormat::PlainText,
            include_metadata: false,
            include_characters: false,
        };

        let result = package_to_plain_text(&package, &options);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("纯文本测试"));
        assert!(text.contains("第 1 集"));
        assert!(text.contains("A: 你好"));
        // 纯文本不应包含 Markdown 或 HTML 标记
        assert!(!text.contains("##"));
        assert!(!text.contains("<h"));
    }

    #[test]
    fn test_export_format_extension_roundtrip() {
        let formats = vec![
            ExportFormat::Json,
            ExportFormat::Markdown,
            ExportFormat::Html,
            ExportFormat::PlainText,
        ];

        for format in formats {
            let ext = format.extension();
            let parsed = ExportFormat::from_extension(ext);
            assert_eq!(parsed, Some(format));
        }
    }

    #[test]
    fn test_empty_package_exports() {
        let package = json!({});
        let options = ExportOptions::default();

        // 所有格式都应该能处理空包
        let md = package_to_markdown(&package, &options);
        assert!(md.is_ok());

        let html = package_to_html(&package, &options);
        assert!(html.is_ok());

        let txt = package_to_plain_text(&package, &options);
        assert!(txt.is_ok());
    }

    #[test]
    fn test_markdown_episode_structure() {
        let package = json!({
            "title": "多集测试",
            "episodes": [
                {
                    "title": "开始",
                    "scenes": [
                        {
                            "description": "场景1",
                            "lines": [
                                {"speaker": "A", "text": "台词1"}
                            ]
                        }
                    ]
                },
                {
                    "title": "继续",
                    "scenes": []
                }
            ]
        });

        let options = ExportOptions::default();
        let result = package_to_markdown(&package, &options);

        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.contains("### 第 1 集：开始"));
        assert!(markdown.contains("### 第 2 集：继续"));
        assert!(markdown.contains("#### 场景 1"));
        assert!(markdown.contains("**A**: 台词1"));
    }
}
