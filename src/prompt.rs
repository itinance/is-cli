pub const SYSTEM_PROMPT: &str = "You answer a yes/no question about the user's current \
environment. Investigate using only the read-only tools available to you. If a command you \
need is denied, work around it or answer \"unknown\". Your final message must be exactly one \
JSON object and nothing else: {\"verdict\": \"yes\"|\"no\"|\"unknown\", \"answer\": \"<one \
concise line explaining the verdict>\"}";

pub const ALLOWED_TOOLS: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    "Bash(git status:*)",
    "Bash(git log:*)",
    "Bash(git branch:*)",
    "Bash(git show:*)",
    "Bash(git diff:*)",
    "Bash(git rev-parse:*)",
    "Bash(git merge-base:*)",
    "Bash(git remote:*)",
    "Bash(git ls-files:*)",
    "Bash(gh pr view:*)",
    "Bash(gh pr list:*)",
    "Bash(gh run view:*)",
    "Bash(gh run list:*)",
    "Bash(date:*)",
    "Bash(uname:*)",
    "Bash(df:*)",
    "Bash(uptime:*)",
    "Bash(which:*)",
];

pub fn build_args(question: &str, model: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-p".into(),
        question.into(),
        "--append-system-prompt".into(),
        SYSTEM_PROMPT.into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
    ];
    if let Some(m) = model {
        args.push("--model".into());
        args.push(m.into());
    }
    args.push("--allowedTools".into());
    args.extend(ALLOWED_TOOLS.iter().map(|s| s.to_string()));
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_full_argv_with_model() {
        let args = build_args("this merged", Some("haiku"));
        let joined = args.join(" ");
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "this merged");
        assert!(joined.contains("--output-format stream-json"));
        assert!(joined.contains("--verbose"));
        assert!(joined.contains("--model haiku"));
        assert!(joined.contains("--append-system-prompt"));
        assert!(!joined.contains("--bare"));
        // allowlist is passed after --allowedTools, one arg per entry
        let idx = args.iter().position(|a| a == "--allowedTools").unwrap();
        assert_eq!(
            &args[idx + 1..],
            ALLOWED_TOOLS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .as_slice()
        );
    }

    #[test]
    fn omits_model_flag_when_inheriting_default() {
        let args = build_args("this merged", None);
        assert!(!args.iter().any(|a| a == "--model"));
    }

    #[test]
    fn allowlist_has_no_mutating_commands() {
        for tool in ALLOWED_TOOLS {
            assert!(!tool.contains("fetch"), "no git fetch allowed: {tool}");
            assert!(!tool.contains("push") && !tool.contains("commit"), "{tool}");
        }
        assert!(!ALLOWED_TOOLS.contains(&"Write"));
        assert!(!ALLOWED_TOOLS.contains(&"Edit"));
    }
}
