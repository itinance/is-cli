use serde_json::Value;

#[derive(Debug, PartialEq)]
pub enum Event {
    ToolUse { name: String, detail: String },
    Result { text: String, is_error: bool },
}

pub fn parse_line(line: &str) -> Vec<Event> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    match v.get("type").and_then(Value::as_str) {
        Some("assistant") => v
            .pointer("/message/content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                    .map(|b| {
                        let name = b.get("name").and_then(Value::as_str).unwrap_or("tool").to_string();
                        let detail = tool_detail(&name, b.get("input"));
                        Event::ToolUse { name, detail }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some("result") => vec![Event::Result {
            text: v.get("result").and_then(Value::as_str).unwrap_or("").to_string(),
            is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
        }],
        _ => Vec::new(),
    }
}

fn tool_detail(name: &str, input: Option<&Value>) -> String {
    let Some(input) = input else { return String::new() };
    let key = match name {
        "Bash" => "command",
        "Read" => "file_path",
        "Grep" | "Glob" => "pattern",
        _ => return String::new(),
    };
    input.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_bash_command_from_tool_use() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"git merge-base HEAD origin/main"}}]}}"#;
        assert_eq!(
            parse_line(line),
            vec![Event::ToolUse {
                name: "Bash".into(),
                detail: "git merge-base HEAD origin/main".into()
            }]
        );
    }

    #[test]
    fn extracts_multiple_tool_uses_and_read_grep_details() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"checking"},{"type":"tool_use","name":"Read","input":{"file_path":"/repo/Cargo.toml"}},{"type":"tool_use","name":"Grep","input":{"pattern":"version"}}]}}"#;
        assert_eq!(
            parse_line(line),
            vec![
                Event::ToolUse { name: "Read".into(), detail: "/repo/Cargo.toml".into() },
                Event::ToolUse { name: "Grep".into(), detail: "version".into() },
            ]
        );
    }

    #[test]
    fn extracts_result_event() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"{\"verdict\": \"yes\", \"answer\": \"ok\"}"}"#;
        assert_eq!(
            parse_line(line),
            vec![Event::Result { text: "{\"verdict\": \"yes\", \"answer\": \"ok\"}".into(), is_error: false }]
        );
    }

    #[test]
    fn ignores_init_text_only_and_garbage_lines() {
        assert!(parse_line(r#"{"type":"system","subtype":"init","session_id":"abc"}"#).is_empty());
        assert!(parse_line(r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#).is_empty());
        assert!(parse_line("not json at all").is_empty());
        assert!(parse_line("").is_empty());
    }
}
