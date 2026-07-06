pub const SYSTEM_PROMPT: &str = "You answer a yes/no question about the user's current \
environment. Investigate using only the read-only tools available to you. To check whether a \
file or directory exists (e.g. an app bundle like Xcode.app, which is a directory), use `ls -d` \
or `stat`, not Read — Read errors the same way for 'missing' and 'is a directory, not a file', \
so it cannot tell those apart. If a command you need is denied, work around it or answer \
\"unknown\". Your final message must be exactly one JSON object and nothing else: \
{\"verdict\": \"yes\"|\"no\"|\"unknown\", \"answer\": \"<one concise line explaining the \
verdict>\"}";

pub const ALLOWED_TOOLS: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    // Fetch a URL, read-only (GET). Enables "is my version the latest release?".
    "WebFetch",
    // git -- read-only inspection. Note the scoped entries below (`stash list`,
    // `config --get`/`--list`, `tag -l`): their bare forms (`git stash`,
    // `git config <k> <v>`, `git tag <name>`) mutate, so only the read prefix
    // is allowed.
    "Bash(git status:*)",
    "Bash(git log:*)",
    "Bash(git branch:*)",
    "Bash(git show:*)",
    "Bash(git diff:*)",
    "Bash(git rev-parse:*)",
    "Bash(git merge-base:*)",
    "Bash(git remote:*)",
    "Bash(git ls-files:*)",
    "Bash(git ls-remote:*)",
    "Bash(git describe:*)",
    "Bash(git blame:*)",
    "Bash(git reflog:*)",
    "Bash(git stash list:*)",
    "Bash(git config --get:*)",
    "Bash(git config --list:*)",
    "Bash(git tag -l:*)",
    // gh -- read-only. `gh api` is deliberately excluded (allows any method).
    "Bash(gh pr view:*)",
    "Bash(gh pr list:*)",
    "Bash(gh pr checks:*)",
    "Bash(gh run view:*)",
    "Bash(gh run list:*)",
    "Bash(gh issue view:*)",
    "Bash(gh issue list:*)",
    "Bash(gh release view:*)",
    "Bash(gh release list:*)",
    "Bash(gh repo view:*)",
    "Bash(gh auth status:*)",
    // System / environment -- read-only queries.
    "Bash(date:*)",
    "Bash(uname:*)",
    "Bash(sw_vers:*)",
    "Bash(df:*)",
    "Bash(du:*)",
    "Bash(uptime:*)",
    "Bash(which:*)",
    "Bash(whoami:*)",
    "Bash(id:*)",
    "Bash(ls:*)",
    "Bash(stat:*)",
    "Bash(test:*)",
    "Bash(file:*)",
    "Bash(readlink:*)",
    // Processes / ports -- "is X running", "is something on :8080".
    "Bash(ps:*)",
    "Bash(pgrep:*)",
    "Bash(lsof:*)",
    // Network -- read-only DNS lookup ("does example.com resolve").
    "Bash(dig:*)",
    // macOS read-only queries. Each is scoped away from its mutating sibling
    // (`xcode-select --install/--switch`, `defaults write`, `pmset <setting>`,
    // `brew install`).
    "Bash(xcode-select -p:*)",
    "Bash(defaults read:*)",
    "Bash(pmset -g:*)",
    "Bash(brew list:*)",
    "Bash(brew outdated:*)",
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
            assert!(!tool.contains("git fetch"), "no git fetch allowed: {tool}");
            assert!(!tool.contains("push"), "no push allowed: {tool}");
            assert!(!tool.contains("commit"), "no commit allowed: {tool}");
        }
        assert!(!ALLOWED_TOOLS.contains(&"Write"));
        assert!(!ALLOWED_TOOLS.contains(&"Edit"));
    }

    #[test]
    fn scoped_commands_exclude_their_mutating_bare_form() {
        // Each bare prefix matches (and would therefore permit) a writing
        // subcommand, so only the read-only scoped form may appear.
        for bare in [
            "Bash(git tag:*)",
            "Bash(git config:*)",
            "Bash(git stash:*)",
            "Bash(brew:*)",
            "Bash(defaults:*)",
            "Bash(pmset:*)",
            "Bash(xcode-select:*)",
        ] {
            assert!(
                !ALLOWED_TOOLS.contains(&bare),
                "too broad, would allow writes: {bare}"
            );
        }
    }

    #[test]
    fn allowlist_has_no_arbitrary_network_or_exec_tools() {
        // WebFetch/dig are read-only network reads and are allowed on purpose;
        // these can write to the network or execute arbitrary code, so they
        // must never appear.
        for tool in ALLOWED_TOOLS {
            for banned in ["curl", "wget", "gh api", "env", "printenv", "xargs"] {
                assert!(!tool.contains(banned), "must not allow {banned}: {tool}");
            }
        }
    }
}
