use clap::Parser;

/// Ask your terminal yes/no questions.
///
/// Everything after the flags is the question: `is this branch already merged`
#[derive(Parser, Debug)]
#[command(name = "is", version, about)]
pub struct Cli {
    /// Escalate to your Claude Code default model for this run
    #[arg(short = 'H', long)]
    pub hard: bool,

    /// Suppress the live action trace
    #[arg(short, long)]
    pub quiet: bool,

    /// Print {"verdict": ..., "answer": ...} JSON on stdout
    #[arg(long)]
    pub json: bool,

    /// One-off model override (not persisted)
    #[arg(long)]
    pub model: Option<String>,

    /// Give up after this many seconds
    #[arg(long, default_value_t = 60)]
    pub timeout: u64,

    /// The question
    #[arg(required = true, trailing_var_arg = true)]
    pub question: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags_before_question() {
        let cli = Cli::try_parse_from(["is", "-H", "-q", "this", "merged"]).unwrap();
        assert!(cli.hard);
        assert!(cli.quiet);
        assert_eq!(cli.question, vec!["this", "merged"]);
        assert_eq!(cli.timeout, 60);
    }

    #[test]
    fn parses_model_and_timeout_values() {
        let cli =
            Cli::try_parse_from(["is", "--model", "opus", "--timeout", "5", "today", "monday"])
                .unwrap();
        assert_eq!(cli.model.as_deref(), Some("opus"));
        assert_eq!(cli.timeout, 5);
        assert_eq!(cli.question, vec!["today", "monday"]);
    }

    #[test]
    fn question_is_required() {
        assert!(Cli::try_parse_from(["is", "--json"]).is_err());
    }
}
