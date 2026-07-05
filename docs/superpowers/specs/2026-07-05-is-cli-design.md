# `is` — ask your terminal yes/no questions

**Date:** 2026-07-05
**Status:** Approved design, pre-implementation
**Crate:** `is-cli` (verified available on crates.io; `is` itself is taken) — installed binary is named `is`
**Language:** Rust

## Purpose

A CLI tool for engineers who live in the terminal. Everything typed after `is` is a
natural-language yes/no question about the current environment; a Claude agent
investigates with read-only commands and returns a verdict.

```
$ is this branch already merged
  ⤷ git branch --show-current
  ⤷ git merge-base HEAD origin/main
yes — feature/login is in origin/main (a1b2c3d)
$ echo $?
0

$ is this merged && git branch -d feature/login
```

`is` wraps the user's locally installed, already-logged-in `claude` CLI in headless
mode (`claude -p`). Claude Code being installed and authenticated is the single
prerequisite; auth (including Pro/Max subscription login), model access, and updates
are inherited from it. `is` never touches credentials itself.

## UX contract

### Invocation

Everything after `is` (that is not a recognized flag) is the question — no quotes
required: `is today monday`. Flags are recognized by leading dash and may appear
before the question.

### Exit codes (grep precedent)

| Code | Meaning |
|------|---------|
| 0 | yes |
| 1 | no |
| 2 | can't determine (verdict "unknown", timeout, unparseable verdict) |
| 3 | operational error (`claude` missing, not logged in, spawn failure, invalid flags) |

### Streams

- **stdout:** the final one-line answer only.
- **stderr:** live action trace (`  ⤷ <command>` per tool call), shown only when
  stderr is a TTY. Pipes and `&&`-chains automatically get clean output.

### Flags

| Flag | Effect |
|------|--------|
| `-H`, `--hard` | Escalate for this run: omit `--model` so the user's Claude Code default model is used |
| `-q`, `--quiet` | Suppress the action trace even on a TTY |
| `--json` | Print `{"verdict": "...", "answer": "..."}` on stdout instead of prose |
| `--model <m>` | One-off model override; **not** persisted |
| `--timeout <secs>` | Kill the agent after N seconds (default 60) and exit 2 |
| `--version`, `--help` | Standard |

## Model selection

Default model: **haiku** (fast and cheap; most is-questions are simple).

Precedence per run:

1. `--model <m>` flag (one-off, not persisted)
2. `-H/--hard` (omit model, inherit user's Claude Code default)
3. Trailing `using <model>` in the question (used **and persisted**, see below)
4. Persisted model from the config file
5. Built-in default `haiku`

### Natural-language switch: `using <model>`

If the question ends with `using <model>`, the phrase is stripped from the question,
that model is used for this run, **and it is written to the config file as the new
default** for future runs:

```
$ is this refactor fully covered by tests using sonnet
  # answers with sonnet, and future `is` calls default to sonnet
```

`<model>` is accepted only if it looks like a model: one of the known aliases
(`haiku`, `sonnet`, `opus`, `fable`) or a `claude-*` model id. Otherwise the words
are treated as part of the question and nothing is stripped. On persist, print a
one-line notice to stderr: `model set to sonnet (stored in ~/.config/is/config.toml)`.

### Config file

`$XDG_CONFIG_HOME/is/config.toml`, falling back to `~/.config/is/config.toml` on all
platforms. Created on first persist. User-editable TOML:

```toml
model = "sonnet"
```

Unknown keys are ignored (forward compatibility). A malformed file produces a
warning on stderr and falls back to defaults — it never blocks answering.

## Architecture

A single Rust binary. Argument parsing with `clap` (trailing free-text args for the
question). The binary spawns the user's local `claude`:

```
claude -p "<question>" \
  --append-system-prompt "<verdict protocol + read-only instructions>" \
  --model <resolved model, unless -H> \
  --output-format stream-json \
  --allowedTools <read-only list>
```

Deliberately **not** `--bare`: bare mode skips OAuth/keychain reads, and subscription
auth (the whole point) requires the normal path. If a future `claude` version makes
bare the default for `-p`, the wrapper passes whatever flag preserves OAuth.

The wrapper consumes stream-json events from the child's stdout:

- `tool_use` events → trace lines on stderr (command extracted from the tool input)
- final `result` event → verdict extraction, exit code, stdout answer

### Verdict protocol

The appended system prompt instructs the agent:

> You answer a yes/no question about the user's current environment. Investigate
> using only the read-only tools available to you. If a needed command is denied,
> work around it or answer "unknown". Your final message must be exactly one JSON
> object and nothing else: `{"verdict": "yes"|"no"|"unknown", "answer": "<one
> concise line explaining the verdict>"}`.

Parsing is strict-first (whole final text is JSON), then a fallback regex extracts
the first `{"verdict": ...}` object from surrounding prose. If both fail: exit 2 and
print the raw final text so the user still gets something.

## Tool policy — the read-only guarantee

Harness-enforced allowlist; the agent cannot mutate anything even if prompted to.
This is a README-grade guarantee: **`is` never changes your system.**

Allowed:

- `Read`, `Grep`, `Glob` (built-in read-only tools)
- `Bash` restricted to patterns:
  - `git status:*`, `git log:*`, `git branch:*`, `git show:*`, `git diff:*`,
    `git rev-parse:*`, `git merge-base:*`, `git remote:*`, `git ls-files:*`
  - `gh pr view:*`, `gh pr list:*`, `gh run view:*`, `gh run list:*`
  - `date:*`, `uname:*`, `df:*`, `uptime:*`, `which:*`
- Everything else (Write, Edit, other Bash) is denied by the permission system; in
  `-p` mode a denied tool call is refused and the agent adapts or answers "unknown".

Notably absent by design: `git fetch` (updates remote-tracking refs — answers about
remote state reflect the last fetch; the answer text should say so when relevant).

## Error handling

| Condition | Behavior |
|-----------|----------|
| `claude` not on PATH | exit 3; message with install instructions |
| Auth failure detected in child output | exit 3; hint to run `claude` and log in |
| Child exits non-zero without result | exit 3; surface child stderr |
| Timeout (default 60s) | kill child process group; exit 2; "couldn't determine within 60s" |
| Unparseable verdict | exit 2; print raw final text |
| Malformed config file | warn on stderr; continue with defaults |

## Testing

- **Unit:** flag/question parsing (including `using <model>` extraction and
  false-positive cases), stream-json event parsing, verdict extraction (fixture
  transcripts as test data), config read/write/merge, model precedence.
- **Integration:** a fake `claude` shim script placed first in `PATH` that replays
  canned stream-json — exercises the full spawn → trace → parse → exit-code path
  with zero API calls, including timeout and auth-failure fixtures.
- **Live smoke tests** (opt-in via env var, excluded from CI): `is today monday`
  and one git question against a fixture repo.

## Distribution

- `cargo install is-cli` (binary `is`)
- Homebrew tap as a follow-up once v0 is stable

## Alternatives considered

- **Claude Agent SDK (TS/Python):** technically supports the same auth stack, but
  SDK docs explicitly disallow third-party products offering claude.ai
  login/subscription rate limits without approval; the documented SDK path is API
  keys. Wrapping the user's own local CLI is the softest form of subscription use.
  Revisit the SDK if an API-key mode is ever added. If `is` becomes a published
  product, seek Anthropic sign-off regardless.
- **Local fast-paths** (answer trivial date/branch questions without an API call):
  deferred, YAGNI for v1; easy to add behind the same UX later.
- **Bash wrapper spike:** rejected — fragile quoting, jq dependency, hard to test.
- **TypeScript/npm wrapper:** viable, rejected in favor of a single static binary
  (instant startup, no runtime dependency, Homebrew/cargo distribution).
