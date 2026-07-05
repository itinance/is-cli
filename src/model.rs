const ALIASES: &[&str] = &["haiku", "sonnet", "opus", "fable"];

pub fn looks_like_model(word: &str) -> bool {
    ALIASES.contains(&word) || word.starts_with("claude-")
}

pub fn extract_using_model(words: &[String]) -> (String, Option<String>) {
    if words.len() >= 2
        && words[words.len() - 2].eq_ignore_ascii_case("using")
        && looks_like_model(&words[words.len() - 1])
    {
        (
            words[..words.len() - 2].join(" "),
            Some(words[words.len() - 1].clone()),
        )
    } else {
        (words.join(" "), None)
    }
}

pub fn resolve_model(
    flag: Option<String>,
    hard: bool,
    using: Option<String>,
    config: Option<String>,
) -> Option<String> {
    if flag.is_some() {
        return flag;
    }
    if hard {
        return None;
    }
    using.or(config).or_else(|| Some("haiku".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn recognizes_aliases_and_claude_ids() {
        assert!(looks_like_model("sonnet"));
        assert!(looks_like_model("haiku"));
        assert!(looks_like_model("claude-sonnet-5"));
        assert!(!looks_like_model("brain"));
        assert!(!looks_like_model("using"));
    }

    #[test]
    fn strips_trailing_using_model() {
        let (q, m) = extract_using_model(&words("this branch merged using sonnet"));
        assert_eq!(q, "this branch merged");
        assert_eq!(m.as_deref(), Some("sonnet"));
    }

    #[test]
    fn using_is_case_insensitive_on_the_keyword() {
        let (q, m) = extract_using_model(&words("this merged USING claude-opus-4-8"));
        assert_eq!(q, "this merged");
        assert_eq!(m.as_deref(), Some("claude-opus-4-8"));
    }

    #[test]
    fn leaves_question_alone_when_last_word_is_not_a_model() {
        let (q, m) = extract_using_model(&words("is this solvable using my brain"));
        assert_eq!(q, "is this solvable using my brain");
        assert_eq!(m, None);
    }

    #[test]
    fn leaves_question_alone_when_using_is_not_second_to_last() {
        let (q, m) = extract_using_model(&words("computed using sonnet yesterday"));
        assert_eq!(q, "computed using sonnet yesterday");
        assert_eq!(m, None);
    }

    #[test]
    fn resolution_precedence() {
        let s = |x: &str| Some(x.to_string());
        // --model flag wins over everything
        assert_eq!(resolve_model(s("opus"), true, s("sonnet"), s("haiku")), s("opus"));
        // -H beats using/config: None = inherit claude default
        assert_eq!(resolve_model(None, true, s("sonnet"), s("haiku")), None);
        // using beats config
        assert_eq!(resolve_model(None, false, s("sonnet"), s("opus")), s("sonnet"));
        // config beats built-in default
        assert_eq!(resolve_model(None, false, None, s("opus")), s("opus"));
        // built-in default
        assert_eq!(resolve_model(None, false, None, None), s("haiku"));
    }
}
