use serde_json::Value;
use std::collections::HashSet;

pub(crate) fn valid_package(schema: &Value, artifact: &Value, expected_episodes: u64) -> bool {
    let Ok(validator) = jsonschema::validator_for(schema) else {
        return false;
    };
    if !validator.is_valid(artifact)
        || artifact["episodes"].as_array().map(|v| v.len() as u64) != Some(expected_episodes)
    {
        return false;
    }
    let mut known = HashSet::new();
    for key in ["logline", "promise"] {
        if let Some(id) = artifact[key]["node_id"].as_str() {
            known.insert(format!("story-package/{id}"));
        }
    }
    for collection in ["characters", "beats", "episodes", "scenes"] {
        for node in artifact[collection].as_array().into_iter().flatten() {
            let Some(id) = node["node_id"].as_str() else {
                return false;
            };
            let parent = format!("story-package/{id}");
            known.insert(parent.clone());
            if collection == "episodes" {
                if let Some(id) = node["end_hook"]["node_id"].as_str() {
                    known.insert(format!("{parent}/{id}"));
                }
            }
            if collection == "scenes" {
                for line in node["lines"].as_array().into_iter().flatten() {
                    if let Some(id) = line["node_id"].as_str() {
                        known.insert(format!("{parent}/{id}"));
                    }
                }
            }
        }
    }
    for collection in ["facts", "relationships", "timeline", "setups"] {
        for node in artifact["continuity_ledger"][collection]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(id) = node["node_id"].as_str() {
                known.insert(format!("story-package/{id}"));
            }
        }
    }
    let mut refs = Vec::new();
    collect_refs(artifact, &mut refs);
    refs.into_iter().all(|r| known.contains(r))
}
fn collect_refs<'a>(value: &'a Value, refs: &mut Vec<&'a str>) {
    match value {
        Value::String(s) if valid_span_ref(s) => refs.push(s),
        Value::Array(xs) => xs.iter().for_each(|x| collect_refs(x, refs)),
        Value::Object(xs) => xs.values().for_each(|x| collect_refs(x, refs)),
        _ => {}
    }
}
pub(crate) fn valid_span_ref(value: &str) -> bool {
    let Some(path) = value.strip_prefix("story-package/") else {
        return false;
    };
    !path.is_empty()
        && path.split('/').all(|segment| {
            let Some((kind, index)) = segment.rsplit_once('-') else {
                return false;
            };
            !kind.is_empty()
                && kind.bytes().all(|b| b.is_ascii_lowercase())
                && !index.starts_with('0')
                && index.parse::<u32>().is_ok_and(|v| v > 0)
        })
}
