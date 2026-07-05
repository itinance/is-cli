use clap::Parser;
use is_cli::{cli, config, model, prompt, runner, verdict};
use std::io::IsTerminal;
use std::time::Duration;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args = cli::Cli::parse();

    let cfg_path = config::config_path();
    let cfg = cfg_path.as_deref().map(config::load).unwrap_or_default();

    let (question, using_model) = model::extract_using_model(&args.question);
    if question.is_empty() {
        eprintln!("error: empty question");
        return 3;
    }

    if let Some(m) = &using_model {
        match &cfg_path {
            Some(p) => {
                let new_cfg = config::Config {
                    model: Some(m.clone()),
                };
                match config::save(p, &new_cfg) {
                    Ok(()) => eprintln!("model set to {m} (stored in {})", p.display()),
                    Err(e) => eprintln!("warning: could not store model: {e}"),
                }
            }
            None => eprintln!("warning: could not determine config directory; model not stored"),
        }
    }

    let resolved = model::resolve_model(args.model.clone(), args.hard, using_model, cfg.model);
    let claude_args = prompt::build_args(&question, resolved.as_deref());
    let trace = !args.quiet && std::io::stderr().is_terminal();

    let outcome = match runner::run_claude(
        "claude",
        &claude_args,
        Duration::from_secs(args.timeout),
        trace,
    ) {
        Ok(o) => o,
        Err(runner::SpawnError::NotFound) => {
            eprintln!(
                "error: `claude` not found on PATH.\n\
                 `is` drives your local Claude Code install. Get it: npm i -g @anthropic-ai/claude-code"
            );
            return 3;
        }
        Err(runner::SpawnError::Io(e)) => {
            eprintln!("error: failed to start claude: {e}");
            return 3;
        }
    };

    if outcome.timed_out {
        eprintln!("couldn't determine within {}s", args.timeout);
        return 2;
    }

    let Some(text) = outcome.final_text else {
        if looks_like_auth_failure(&outcome.stderr) {
            eprintln!("error: claude is not logged in. Run `claude` and use /login first.");
        } else {
            eprintln!("error: claude exited without a result");
            if !outcome.stderr.is_empty() {
                eprint!("{}", outcome.stderr);
            }
        }
        return 3;
    };

    if (outcome.is_error || !outcome.status_ok)
        && (looks_like_auth_failure(&text) || looks_like_auth_failure(&outcome.stderr))
    {
        eprintln!("error: claude is not logged in. Run `claude` and use /login first.");
        return 3;
    }

    match verdict::parse_verdict(&text) {
        Some((v, answer)) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::json!({ "verdict": v.as_str(), "answer": answer })
                );
            } else {
                println!("{} — {}", v.as_str(), answer);
            }
            v.exit_code()
        }
        None => {
            println!("{text}");
            2
        }
    }
}

fn looks_like_auth_failure(s: &str) -> bool {
    let s = s.to_lowercase();
    s.contains("not logged in")
        || s.contains("/login")
        || s.contains("authentication")
        || s.contains("oauth")
        || s.contains("api key")
}
