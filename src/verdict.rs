use serde::Deserialize;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Verdict {
    Yes,
    No,
    Unknown,
}

impl Verdict {
    pub fn exit_code(self) -> i32 {
        match self {
            Verdict::Yes => 0,
            Verdict::No => 1,
            Verdict::Unknown => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Yes => "yes",
            Verdict::No => "no",
            Verdict::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawVerdict {
    verdict: String,
    answer: String,
}

pub fn parse_verdict(text: &str) -> Option<(Verdict, String)> {
    let trimmed = text.trim();
    if let Some(v) = serde_json::from_str::<RawVerdict>(trimmed)
        .ok()
        .and_then(from_raw)
    {
        return Some(v);
    }
    // Fallback: first parseable JSON object with a valid verdict, embedded in prose.
    for (i, _) in trimmed.match_indices('{') {
        let mut stream =
            serde_json::Deserializer::from_str(&trimmed[i..]).into_iter::<RawVerdict>();
        if let Some(Ok(raw)) = stream.next() {
            if let Some(v) = from_raw(raw) {
                return Some(v);
            }
        }
    }
    None
}

fn from_raw(raw: RawVerdict) -> Option<(Verdict, String)> {
    let verdict = match raw.verdict.as_str() {
        "yes" => Verdict::Yes,
        "no" => Verdict::No,
        "unknown" => Verdict::Unknown,
        _ => return None,
    };
    Some((verdict, raw.answer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_grep_convention() {
        assert_eq!(Verdict::Yes.exit_code(), 0);
        assert_eq!(Verdict::No.exit_code(), 1);
        assert_eq!(Verdict::Unknown.exit_code(), 2);
    }

    #[test]
    fn parses_strict_json() {
        let (v, a) = parse_verdict(r#"{"verdict": "yes", "answer": "branch is merged"}"#).unwrap();
        assert_eq!(v, Verdict::Yes);
        assert_eq!(a, "branch is merged");
    }

    #[test]
    fn parses_json_with_surrounding_whitespace_and_extra_keys() {
        let (v, _) =
            parse_verdict("  {\"verdict\": \"no\", \"answer\": \"nope\", \"confidence\": 0.9}\n")
                .unwrap();
        assert_eq!(v, Verdict::No);
    }

    #[test]
    fn falls_back_to_json_embedded_in_prose() {
        let text = r#"Here is my answer: {"verdict": "unknown", "answer": "no remote configured"} — hope that helps."#;
        let (v, a) = parse_verdict(text).unwrap();
        assert_eq!(v, Verdict::Unknown);
        assert_eq!(a, "no remote configured");
    }

    #[test]
    fn rejects_invalid_verdict_values_and_plain_prose() {
        assert!(parse_verdict(r#"{"verdict": "maybe", "answer": "hm"}"#).is_none());
        assert!(parse_verdict("Yes, it is merged.").is_none());
        assert!(parse_verdict("").is_none());
    }
}
