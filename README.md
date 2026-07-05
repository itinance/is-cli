# is

Ask your terminal yes/no questions.

```text
$ is this branch already merged
  ⤷ git branch --show-current
  ⤷ git merge-base HEAD origin/main
yes — feature/login is in origin/main (a1b2c3d)

$ is this merged && git branch -d feature/login
```

Everything after `is` is a natural-language question. A Claude agent investigates
your current environment with **read-only** commands and answers with a verdict.

## Install

```sh
cargo install is-cli   # installs the `is` binary
```

**Prerequisite:** a logged-in [Claude Code](https://claude.com/claude-code) install
(`claude` on your PATH). `is` drives it headlessly and inherits its auth — including
Claude Pro/Max subscription login. `is` never touches your credentials.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | yes |
| 1 | no |
| 2 | can't determine |
| 3 | operational error (claude missing / not logged in) |

So `is` composes: `is this merged && git branch -d old`.

## Flags

- `-H, --hard` — escalate to your Claude Code default model for one run
- `-q, --quiet` — no action trace
- `--json` — `{"verdict": "...", "answer": "..."}` on stdout
- `--model <m>` — one-off model override
- `--timeout <secs>` — default 60

## Models

Default is `haiku` (fast, cheap). End any question with `using <model>` to switch
**and persist** it for future runs:

```sh
is this refactor fully covered by tests using sonnet
```

Stored in `~/.config/is/config.toml` (edit or delete freely).

## A note on subscription auth

`is` deliberately wraps your locally installed `claude` CLI instead of calling the
Claude API directly. Questions therefore run on whatever auth your Claude Code
login uses — for Pro/Max users, that's your subscription, with no separate API key
or per-token billing.

This works because Claude Code's headless mode (`claude -p`) inherits the normal
login path. It is **not** a contractual guarantee: whether subscription quota may
be consumed this way is ultimately Anthropic's call, and a future Claude Code
version or policy change could restrict headless subscription use. If that
happens, `is` would need an API-key mode (or your Claude Code setup would need
one) to keep working. Until then: your login, your quota, your usage.

## The read-only guarantee

The agent runs with a hard allowlist enforced by Claude Code's permission system —
`git` inspection subcommands, `gh pr/run view`, `date`, and file reads. It cannot
write, edit, push, or even `git fetch`. **`is` never changes your system.**

## Development

```sh
cargo test                                   # unit + integration tests
cargo llvm-cov --open                        # coverage report (requires cargo-llvm-cov)
```
