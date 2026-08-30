//! Export a `story-package/v1` artifact to human-readable formats.
//!
//! The converters read the real package schema (logline, promise, characters,
//! beats, episodes, top-level scenes, typed lines) — not the legacy shape that
//! used `title`/`premise`/`episode.scenes`.

use serde_json::Value;
use std::collections::HashMap;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    pub include_metadata: bool,
    pub include_characters: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_metadata: true,
            include_characters: true,
        }
    }
}

// --- field access helpers -------------------------------------------------

/// Non-blank string at `value[key]`.
fn text_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn logline_text(package: &Value) -> Option<&str> {
    package
        .get("logline")
        .and_then(|logline| text_of(logline, "text"))
}

/// `node_id` (e.g. `char-1`) -> display `name` for every character.
fn character_names(package: &Value) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for character in package
        .get("characters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(node_id), Some(name)) =
            (text_of(character, "node_id"), text_of(character, "name"))
        {
            names.insert(node_id.to_owned(), name.to_owned());
        }
    }
    names
}

/// Resolve a `story-package/char-N` span reference to a character name.
fn resolve_speaker(names: &HashMap<String, String>, speaker: &str) -> String {
    let key = speaker.strip_prefix("story-package/").unwrap_or(speaker);
    names
        .get(key)
        .cloned()
        .unwrap_or_else(|| speaker.to_owned())
}

/// `story-package/ep-N` -> N.
fn episode_number(episode_ref: &str) -> Option<u32> {
    episode_ref
        .strip_prefix("story-package/ep-")?
        .parse::<u32>()
        .ok()
}

fn episode_label(episode_ref: &str) -> String {
    episode_number(episode_ref)
        .map(|number| format!("第 {number} 集"))
        .unwrap_or_else(|| episode_ref.to_owned())
}

/// Episode `index` is an integer in the real schema; tolerate a string too.
fn episode_index(episode: &Value) -> String {
    match episode.get("index") {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::String(text)) if !text.trim().is_empty() => text.trim().to_owned(),
        _ => "?".to_owned(),
    }
}

// --- Markdown -------------------------------------------------------------

pub fn package_to_markdown(package: &Value, options: &ExportOptions) -> Result<String, String> {
    let mut output = String::new();
    let names = character_names(package);

    if let Some(title) = logline_text(package) {
        output.push_str(&format!("# {title}\n\n"));
    }

    if options.include_metadata {
        output.push_str("## 故事信息\n\n");
        let promise = package.get("promise");
        if let Some(genre) = promise.and_then(|value| text_of(value, "genre")) {
            output.push_str(&format!("**类型**: {genre}\n\n"));
        }
        if let Some(audience) = promise.and_then(|value| text_of(value, "audience")) {
            output.push_str(&format!("**受众**: {audience}\n\n"));
        }
        if let Some(tone) = promise.and_then(|value| text_of(value, "tone")) {
            output.push_str(&format!("**基调**: {tone}\n\n"));
        }
        if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
            output.push_str(&format!("**集数**: {} 集\n\n", episodes.len()));
        }
        let locations = package
            .get("production")
            .and_then(|production| production.get("locations"))
            .and_then(Value::as_array);
        if let Some(locations) = locations {
            let locations = locations
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            if !locations.is_empty() {
                output.push_str(&format!("**拍摄地点**: {}\n\n", locations.join("、")));
            }
        }
        output.push_str("---\n\n");
    }

    if options.include_characters {
        if let Some(characters) = package.get("characters").and_then(Value::as_array) {
            if !characters.is_empty() {
                output.push_str("## 人物介绍\n\n");
                for character in characters {
                    let Some(name) = text_of(character, "name") else {
                        continue;
                    };
                    output.push_str(&format!("### {name}\n\n"));
                    for (label, key) in [
                        ("欲望", "desire"),
                        ("恐惧", "fear"),
                        ("矛盾", "contradiction"),
                        ("秘密", "secret"),
                        ("转变", "change"),
                    ] {
                        if let Some(value) = text_of(character, key) {
                            output.push_str(&format!("**{label}**: {value}\n\n"));
                        }
                    }
                    if let Some(markers) = character.get("voice_markers").and_then(Value::as_array)
                    {
                        let markers = markers
                            .iter()
                            .filter_map(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .collect::<Vec<_>>();
                        if !markers.is_empty() {
                            output.push_str(&format!("**语气特征**: {}\n\n", markers.join("、")));
                        }
                    }
                }
                output.push_str("---\n\n");
            }
        }
    }

    if let Some(beats) = package.get("beats").and_then(Value::as_array) {
        if !beats.is_empty() {
            output.push_str("## 情节节拍\n\n");
            for (index, beat) in beats.iter().enumerate() {
                let pressure = text_of(beat, "pressure").unwrap_or("（未提供压力）");
                let choice = text_of(beat, "choice").unwrap_or("（未提供选择）");
                let consequence = text_of(beat, "consequence").unwrap_or("（未提供后果）");
                output.push_str(&format!(
                    "{}. **{pressure}** → {choice} → {consequence}\n",
                    index + 1
                ));
            }
            output.push_str("\n---\n\n");
        }
    }

    if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
        output.push_str("## 剧集内容\n\n");
        for episode in episodes {
            let index = episode_index(episode);
            output.push_str(&format!("### 第 {index} 集\n\n"));
            if let Some(opening_state) = text_of(episode, "opening_state") {
                output.push_str(&format!("**开场**: {opening_state}\n\n"));
            }
            if let Some(conflict) = text_of(episode, "conflict") {
                output.push_str(&format!("**冲突**: {conflict}\n\n"));
            }
            if let Some(turn) = text_of(episode, "turn") {
                output.push_str(&format!("**转折**: {turn}\n\n"));
            }
            if let Some(hook_text) = episode
                .get("end_hook")
                .and_then(|hook| text_of(hook, "text"))
            {
                output.push_str(&format!("**结尾钩子**: {hook_text}\n\n"));
            }
        }
        output.push_str("---\n\n");
    }

    if let Some(scenes) = package.get("scenes").and_then(Value::as_array) {
        output.push_str("## 场景\n\n");
        for (index, scene) in scenes.iter().enumerate() {
            let location = text_of(scene, "location").unwrap_or("未注明地点");
            let episode = scene
                .get("episode_ref")
                .and_then(Value::as_str)
                .map(episode_label)
                .unwrap_or_else(|| "未知集数".to_owned());
            output.push_str(&format!(
                "### 场景 {} · {episode} · {location}\n\n",
                index + 1
            ));
            if let Some(lines) = scene.get("lines").and_then(Value::as_array) {
                for line in lines {
                    let kind = text_of(line, "kind").unwrap_or("action");
                    let text = text_of(line, "text").unwrap_or("");
                    if kind == "dialogue" {
                        let speaker = line
                            .get("speaker")
                            .and_then(Value::as_str)
                            .map(|speaker| resolve_speaker(&names, speaker))
                            .unwrap_or_else(|| "（未署名）".to_owned());
                        output.push_str(&format!("**{speaker}**：{text}\n\n"));
                        if let Some(subtext) = text_of(line, "subtext") {
                            output.push_str(&format!("*潜台词：{subtext}*\n\n"));
                        }
                    } else {
                        output.push_str(&format!("_[{text}]_\n\n"));
                    }
                }
            }
            output.push('\n');
        }
    }

    output.push_str("---\n\n*故事完*\n");
    Ok(output)
}

// --- Plain text -----------------------------------------------------------

pub fn package_to_plain_text(package: &Value, options: &ExportOptions) -> Result<String, String> {
    let mut output = String::new();
    let names = character_names(package);

    if let Some(title) = logline_text(package) {
        output.push_str(title);
        output.push('\n');
        output.push_str(&"=".repeat(title.chars().count()));
        output.push_str("\n\n");
    }

    if options.include_metadata {
        if let Some(genre) = package
            .get("promise")
            .and_then(|value| text_of(value, "genre"))
        {
            output.push_str(&format!("类型: {genre}\n"));
        }
        if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
            output.push_str(&format!("集数: {} 集\n\n", episodes.len()));
        }
    }

    if options.include_characters {
        if let Some(characters) = package.get("characters").and_then(Value::as_array) {
            if !characters.is_empty() {
                output.push_str("人物介绍\n");
                output.push_str("---------\n\n");
                for character in characters {
                    let Some(name) = text_of(character, "name") else {
                        continue;
                    };
                    output.push_str(name);
                    output.push('\n');
                    if let Some(desire) = text_of(character, "desire") {
                        output.push_str(&format!("  欲望: {desire}\n"));
                    }
                    if let Some(secret) = text_of(character, "secret") {
                        output.push_str(&format!("  秘密: {secret}\n\n"));
                    }
                }
                output.push('\n');
            }
        }
    }

    if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
        for episode in episodes {
            let index = episode_index(episode);
            output.push_str(&format!("\n第 {index} 集\n"));
            output.push_str(&"-".repeat(20));
            output.push_str("\n\n");
            if let Some(opening_state) = text_of(episode, "opening_state") {
                output.push_str(&format!("开场: {opening_state}\n"));
            }
            if let Some(hook_text) = episode
                .get("end_hook")
                .and_then(|hook| text_of(hook, "text"))
            {
                output.push_str(&format!("结尾钩子: {hook_text}\n\n"));
            }
        }
    }

    if let Some(scenes) = package.get("scenes").and_then(Value::as_array) {
        output.push_str("\n场景\n----\n\n");
        for (index, scene) in scenes.iter().enumerate() {
            let location = text_of(scene, "location").unwrap_or("未注明地点");
            output.push_str(&format!("场景 {} · {location}\n", index + 1));
            if let Some(lines) = scene.get("lines").and_then(Value::as_array) {
                for line in lines {
                    let kind = text_of(line, "kind").unwrap_or("action");
                    let text = text_of(line, "text").unwrap_or("");
                    if kind == "dialogue" {
                        let speaker = line
                            .get("speaker")
                            .and_then(Value::as_str)
                            .map(|speaker| resolve_speaker(&names, speaker))
                            .unwrap_or_else(|| "（未署名）".to_owned());
                        output.push_str(&format!("{speaker}: {text}\n"));
                        if let Some(subtext) = text_of(line, "subtext") {
                            output.push_str(&format!("  （潜台词：{subtext}）\n"));
                        }
                    } else {
                        output.push_str(&format!("[{text}]\n"));
                    }
                }
            }
            output.push('\n');
        }
    }

    output.push_str("\n--- 完 ---\n");
    Ok(output)
}

// --- HTML -----------------------------------------------------------------

pub fn package_to_html(package: &Value, options: &ExportOptions) -> Result<String, String> {
    let mut output = String::new();
    let names = character_names(package);

    output.push_str("<!DOCTYPE html>\n");
    output.push_str("<html lang=\"zh-CN\">\n");
    output.push_str("<head>\n");
    output.push_str("  <meta charset=\"UTF-8\">\n");
    output
        .push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");

    if let Some(title) = logline_text(package) {
        output.push_str(&format!("  <title>{}</title>\n", html_escape(title)));
    }

    output.push_str("  <style>\n");
    output.push_str("    body { font-family: 'Microsoft YaHei', sans-serif; line-height: 1.8; max-width: 800px; margin: 0 auto; padding: 20px; }\n");
    output.push_str(
        "    h1 { color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 10px; }\n",
    );
    output.push_str("    h2 { color: #34495e; margin-top: 40px; }\n");
    output.push_str("    h3 { color: #7f8c8d; }\n");
    output.push_str("    .metadata { background: #ecf0f1; padding: 15px; border-radius: 5px; margin: 20px 0; }\n");
    output.push_str("    .character { margin: 20px 0; padding: 15px; border-left: 4px solid #3498db; background: #f8f9fa; }\n");
    output.push_str("    .episode { margin: 30px 0; }\n");
    output.push_str(
        "    .scene { margin: 20px 0; padding: 15px; background: #fafafa; border-radius: 5px; }\n",
    );
    output.push_str("    .dialogue { margin: 10px 0; }\n");
    output.push_str("    .speaker { font-weight: bold; color: #2980b9; }\n");
    output.push_str("    .subtext { color: #95a5a6; font-style: italic; }\n");
    output.push_str("    .action { font-style: italic; color: #7f8c8d; }\n");
    output.push_str("  </style>\n");
    output.push_str("</head>\n");
    output.push_str("<body>\n");

    if let Some(title) = logline_text(package) {
        output.push_str(&format!("  <h1>{}</h1>\n", html_escape(title)));
    }

    if options.include_metadata {
        output.push_str("  <div class=\"metadata\">\n");
        output.push_str("    <h2>故事信息</h2>\n");
        if let Some(genre) = package
            .get("promise")
            .and_then(|value| text_of(value, "genre"))
        {
            output.push_str(&format!(
                "    <p><strong>类型</strong>: {}</p>\n",
                html_escape(genre)
            ));
        }
        if let Some(tone) = package
            .get("promise")
            .and_then(|value| text_of(value, "tone"))
        {
            output.push_str(&format!(
                "    <p><strong>基调</strong>: {}</p>\n",
                html_escape(tone)
            ));
        }
        if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
            output.push_str(&format!(
                "    <p><strong>集数</strong>: {} 集</p>\n",
                episodes.len()
            ));
        }
        output.push_str("  </div>\n");
    }

    if options.include_characters {
        if let Some(characters) = package.get("characters").and_then(Value::as_array) {
            if !characters.is_empty() {
                output.push_str("  <h2>人物介绍</h2>\n");
                for character in characters {
                    let Some(name) = text_of(character, "name") else {
                        continue;
                    };
                    output.push_str("  <div class=\"character\">\n");
                    output.push_str(&format!("    <h3>{}</h3>\n", html_escape(name)));
                    for (label, key) in [
                        ("欲望", "desire"),
                        ("恐惧", "fear"),
                        ("矛盾", "contradiction"),
                        ("秘密", "secret"),
                        ("转变", "change"),
                    ] {
                        if let Some(value) = text_of(character, key) {
                            output.push_str(&format!(
                                "    <p><strong>{label}</strong>: {}</p>\n",
                                html_escape(value)
                            ));
                        }
                    }
                    output.push_str("  </div>\n");
                }
            }
        }
    }

    if let Some(episodes) = package.get("episodes").and_then(Value::as_array) {
        output.push_str("  <h2>剧集内容</h2>\n");
        for episode in episodes {
            output.push_str("  <div class=\"episode\">\n");
            let index = episode_index(episode);
            output.push_str(&format!("    <h3>第 {index} 集</h3>\n"));
            if let Some(opening_state) = text_of(episode, "opening_state") {
                output.push_str(&format!(
                    "    <p><strong>开场</strong>: {}</p>\n",
                    html_escape(opening_state)
                ));
            }
            if let Some(conflict) = text_of(episode, "conflict") {
                output.push_str(&format!(
                    "    <p><strong>冲突</strong>: {}</p>\n",
                    html_escape(conflict)
                ));
            }
            if let Some(turn) = text_of(episode, "turn") {
                output.push_str(&format!(
                    "    <p><strong>转折</strong>: {}</p>\n",
                    html_escape(turn)
                ));
            }
            if let Some(hook_text) = episode
                .get("end_hook")
                .and_then(|hook| text_of(hook, "text"))
            {
                output.push_str(&format!(
                    "    <p><strong>结尾钩子</strong>: {}</p>\n",
                    html_escape(hook_text)
                ));
            }
            output.push_str("  </div>\n");
        }
    }

    if let Some(scenes) = package.get("scenes").and_then(Value::as_array) {
        output.push_str("  <h2>场景</h2>\n");
        for (index, scene) in scenes.iter().enumerate() {
            output.push_str("  <div class=\"scene\">\n");
            let location = text_of(scene, "location").unwrap_or("未注明地点");
            output.push_str(&format!(
                "    <h3>场景 {} · {}</h3>\n",
                index + 1,
                html_escape(location)
            ));
            if let Some(lines) = scene.get("lines").and_then(Value::as_array) {
                for line in lines {
                    let kind = text_of(line, "kind").unwrap_or("action");
                    let text = text_of(line, "text").unwrap_or("");
                    if kind == "dialogue" {
                        let speaker = line
                            .get("speaker")
                            .and_then(Value::as_str)
                            .map(|speaker| resolve_speaker(&names, speaker))
                            .unwrap_or_else(|| "（未署名）".to_owned());
                        output.push_str("    <div class=\"dialogue\">\n");
                        output.push_str(&format!(
                            "      <span class=\"speaker\">{}:</span> {}\n",
                            html_escape(&speaker),
                            html_escape(text)
                        ));
                        if let Some(subtext) = text_of(line, "subtext") {
                            output.push_str(&format!(
                                "      <div class=\"subtext\">潜台词：{}</div>\n",
                                html_escape(subtext)
                            ));
                        }
                        output.push_str("    </div>\n");
                    } else {
                        output.push_str(&format!(
                            "    <p class=\"action\">[{text}]</p>\n",
                            text = html_escape(text)
                        ));
                    }
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
#[path = "export_formats_tests.rs"]
mod tests;
