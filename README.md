# nu-sidecar

Two nushell plugins:

- **bashify** -- translates common bash idioms (`&&`, `||`, `export`, `$(...)`,
  `2>&1`, `[[ ... ]]`, ...) into nushell syntax. A custom Enter keybinding
  translates your line and shows it back to you; press Enter again (unedited)
  to actually run it. Anything it doesn't recognize is left untouched.
- **llmchat** -- `ask` and `plan` commands backed by a locally- or
  remotely-hosted LLM (Ollama, LM Studio, OpenRouter, Hermes, or OpenClaw),
  configured once via `llm setup`. Recent terminal activity (commands +
  output) is captured automatically and sent along as context, capped by
  size and optionally by `--last N` turns.

## Install

Requires Rust/cargo and a matching nushell version (currently pinned to
`0.115.1` -- see below if you're on a different version).

```
nu install.nu
```

This builds and `cargo install`s both plugins, registers them with nushell
(`plugin add` + `plugin use`), and appends a marker-delimited block to your
`config.nu` that sources [`nu/sidecar.nu`](nu/sidecar.nu) (the keybinding +
hooks). Safe to re-run.

Then, in a new nushell session:

```
llm setup       # one-time: pick a provider and model
ask "hello"
```

## Version pinning

`nu-plugin`/`nu-protocol` are pinned to `=0.115.1` in `Cargo.toml` (nushell's
plugin protocol requires the plugin's crate version to match the running
`nu` binary's version family). If you upgrade `nu`, bump those versions here
and re-run `cargo install --path ... --force` for both plugins.

## bashify: what's translated (V1)

Quote-respecting textual rules, not a full shell grammar parser --
`if`/`for`/`while`/`case` blocks and per-tool flag differences are
deliberately out of scope. Anything a rule doesn't recognize passes through
unchanged; see `crates/nu_plugin_bashify/src/rules.rs` for the full list and
`cargo test -p nu_plugin_bashify` for the fixture-pair test suite.

| bash | nu |
|---|---|
| `cmd1 && cmd2` | `cmd1; if $env.LAST_EXIT_CODE == 0 { cmd2 }` |
| `cmd1 \|\| cmd2` | `cmd1; if $env.LAST_EXIT_CODE != 0 { cmd2 }` |
| `` `cmd` `` / `$(cmd)` | `(cmd)` |
| `export VAR=val` (and later `$VAR` reads in the same chain) | `$env.VAR = "val"` (and `$env.VAR`) |
| `VAR=val cmd args` | `with-env {VAR: "val"} { cmd args }` |
| `unset VAR` | `hide-env VAR` |
| `cmd > file 2>&1` | `cmd o+e> file` |
| `cmd > file` / `>>` / `< file` | `cmd o> file` / `o>>` / `open file \| cmd` |
| `[[ -f/-d/-z/-n/==/!= ]]` | `path exists` / `path type == dir` / `== ""` / `!= ""` / `==` / `!=` |
| `alias x='y'` | `alias x = y` |
| `. file` | `source file` |

Known gap: `export`ed values are emitted as literal nu strings, not
interpolated -- `export PATH=$PATH:/x` won't expand `$PATH` (that needs nu's
`$"...(...)"` syntax, and reliably detecting which `$VAR`s are worth
expanding textually is out of scope).

## llmchat: config and context

- Config: `~/.config/nu-sidecar/config.toml` (TOML, `0600` permissions).
  Providers mirror Sauron's `LLMProviderKind`: `ollama`, `lmStudio`,
  `openRouter`, `hermes`, `openClaw` -- see `llm setup --help`. API keys are
  stored in plaintext (no OS keychain integration for a nushell plugin --
  known limitation).
- Requests are non-streaming (`stream: false`) against each provider's
  OpenAI-compatible `/v1/chat/completions` endpoint.
- Context: a `pre_execution` / `display_output` / `pre_prompt` hook chain
  (see `nu/sidecar.nu`) logs each command + its output to
  `~/.local/state/nu-sidecar/sessions/<pid>.jsonl`, since nushell only keeps
  *input* history natively. External-command output isn't captured (nushell
  limitation -- `display_output` doesn't fire for external processes), and
  shows up as `[external command -- stdout not captured]` instead. Per-entry
  and total-context character budgets are configurable under `[transcript]`
  in the config file; `--last N` additionally caps by turn count.
- `ask`/`plan` are `def`s in `nu/sidecar.nu` that wrap the plugin's
  `sidecar ask`/`sidecar plan` commands, injecting `$nu.pid` so each session
  finds its own transcript automatically.

## Known limitations

- The `display_output` hook is replaced wholesale by `nu/sidecar.nu` (to
  also capture context) -- if you'd customized it before installing
  nu-sidecar, re-add that customization in `nu/sidecar.nu`.
- API keys are plaintext on disk.
- bashify doesn't translate bash block constructs (`if`/`for`/`while`/`case`)
  or per-tool flag differences -- only shell-grammar-level idioms.
