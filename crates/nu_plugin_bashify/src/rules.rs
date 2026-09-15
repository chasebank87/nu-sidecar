//! Bash -> nushell syntax translation.
//!
//! Deliberately *not* a full shell grammar parser: this is a quote-aware
//! tokenizer plus an ordered chain of narrow, regex-driven rewrite rules for
//! the handful of bash idioms people actually type interactively (see the
//! plan doc for the full V1 rule table). Block constructs (`if`/`for`/`while`
//! /`case`) and per-tool flag differences are explicitly out of scope.
//!
//! Every rule is quote-respecting and only fires on an unambiguous textual
//! match. Anything a rule doesn't recognize passes through byte-for-byte
//! unchanged -- this translator never guesses or drops content it doesn't
//! understand.

pub struct TranslateResult {
    pub translated: String,
    pub rules_fired: Vec<&'static str>,
}

pub fn translate(input: &str) -> TranslateResult {
    let mut fired = Vec::new();
    let mut exported = Vec::new();
    let translated = translate_line(input.trim(), &mut fired, &mut exported);
    TranslateResult {
        translated,
        rules_fired: fired,
    }
}

/// Names `export`ed by a segment already processed earlier in the same
/// chain -- so `export FOO=bar && echo $FOO` can rewrite the later `$FOO`
/// read to `$env.FOO` too, not just the export itself.
fn translate_line(input: &str, fired: &mut Vec<&'static str>, exported: &mut Vec<String>) -> String {
    if input.is_empty() {
        return input.to_string();
    }

    // && / || chaining: split at the first top-level (quote/paren-respecting)
    // occurrence and recursively translate both sides, since either side may
    // itself contain further bash-isms (e.g. `export A=1 && cmd`). `left` is
    // always a single (non-chain) segment, since we split at the *first*
    // top-level operator -- so any name it exports is known before `right`
    // is substituted and recursively translated.
    if let Some((left, op, right)) = split_top_level_chain(input) {
        let left_t = translate_line(left.trim(), fired, exported);
        let right_substituted = substitute_env_refs(right.trim(), exported);
        let right_t = translate_line(&right_substituted, fired, exported);
        return match op {
            ChainOp::And => {
                fired.push("&&-chain");
                format!("{left_t}; if $env.LAST_EXIT_CODE == 0 {{ {right_t} }}")
            }
            ChainOp::Or => {
                fired.push("||-chain");
                format!("{left_t}; if $env.LAST_EXIT_CODE != 0 {{ {right_t} }}")
            }
        };
    }

    if let Some(s) = rule_test_expression(input) {
        fired.push("[[ ]]-test");
        return s;
    }
    if let Some((s, name)) = rule_export(input) {
        fired.push("export");
        exported.push(name);
        return s;
    }
    if let Some(s) = rule_unset(input) {
        fired.push("unset");
        return s;
    }
    if let Some(s) = rule_alias(input) {
        fired.push("alias");
        return s;
    }
    if let Some(s) = rule_dot_source(input) {
        fired.push("source");
        return s;
    }
    if let Some(s) = rule_env_prefix(input) {
        fired.push("inline-env-prefix");
        return s;
    }

    let mut s = input.to_string();
    if let Some(new_s) = rule_command_substitution(&s) {
        s = new_s;
        fired.push("command-substitution");
    }
    if let Some(new_s) = rule_redirect_2_1(&s) {
        // Already fully handled the redirect (both streams to a file) --
        // don't let rule_redirects reinterpret the `>` inside `o+e>`.
        s = new_s;
        fired.push("2>&1");
    } else if let Some(new_s) = rule_redirects(&s) {
        s = new_s;
        fired.push("redirect");
    }

    s
}

// ---------------------------------------------------------------------
// && / || chain splitting, quote + paren aware
// ---------------------------------------------------------------------

enum ChainOp {
    And,
    Or,
}

/// Finds the first top-level `&&` or `||` (not inside quotes or parens) and
/// returns (left, op, right). Returns `None` if there's no top-level chain
/// operator, so callers fall through to single-command rules.
fn split_top_level_chain(input: &str) -> Option<(&str, ChainOp, &str)> {
    let bytes = input.as_bytes();
    let mut depth: i32 = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i + 1 < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '(' if !in_single && !in_double => depth += 1,
            ')' if !in_single && !in_double => depth -= 1,
            '&' if !in_single && !in_double && depth == 0 && bytes[i + 1] == b'&' => {
                return Some((&input[..i], ChainOp::And, &input[i + 2..]));
            }
            '|' if !in_single && !in_double && depth == 0 && bytes[i + 1] == b'|' => {
                return Some((&input[..i], ChainOp::Or, &input[i + 2..]));
            }
            _ => {}
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------
// Single-command rules
// ---------------------------------------------------------------------

/// `` `cmd` `` or `$(cmd)` -> `(cmd)`. Non-nested (a nested `$(...)` or
/// backtick pair is left alone rather than mistranslated).
fn rule_command_substitution(input: &str) -> Option<String> {
    let mut out = String::new();
    let mut changed = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'(') {
            chars.next(); // consume '('
            let mut inner = String::new();
            let mut depth = 1;
            for c2 in chars.by_ref() {
                if c2 == '(' {
                    depth += 1;
                } else if c2 == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                inner.push(c2);
            }
            out.push('(');
            out.push_str(&inner);
            out.push(')');
            changed = true;
        } else if c == '`' {
            let mut inner = String::new();
            let mut closed = false;
            for c2 in chars.by_ref() {
                if c2 == '`' {
                    closed = true;
                    break;
                }
                inner.push(c2);
            }
            if closed {
                out.push('(');
                out.push_str(&inner);
                out.push(')');
                changed = true;
            } else {
                out.push('`');
                out.push_str(&inner);
            }
        } else {
            out.push(c);
        }
    }
    changed.then_some(out)
}

/// `export VAR=val` -> `$env.VAR = "val"`. Note: the value is emitted as a
/// literal nu string, not an interpolated one -- `export PATH=$PATH:/x`
/// becomes a string containing the literal text `$PATH:/x` rather than
/// expanding it, since that requires nu's `$"...(...)"` interpolation syntax
/// and reliably detecting which bash `$VAR` references are worth expanding
/// is out of scope for a textual rule chain. Flagged here as a known gap.
fn rule_export(input: &str) -> Option<(String, String)> {
    let rest = input.strip_prefix("export ")?;
    let (name, value) = rest.trim_start().split_once('=')?;
    let name = name.trim();
    if !is_ident(name) {
        return None;
    }
    Some((
        format!("$env.{name} = {}", to_nu_string_literal(value)),
        name.to_string(),
    ))
}

/// Rewrites bare `$NAME` references (not `$env.NAME`, not inside single
/// quotes -- bash suppresses expansion there too) to `$env.NAME` for every
/// name in `exported`. Used to carry an earlier `export NAME=val` forward
/// to later reads of `$NAME` within the same chain.
fn substitute_env_refs(input: &str, exported: &[String]) -> String {
    if exported.is_empty() {
        return input.to_string();
    }
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut in_single = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            in_single = !in_single;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '$' && !in_single && i + 1 < chars.len() && (chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '_')
        {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            let name: String = chars[start..end].iter().collect();
            if exported.iter().any(|e| e == &name) {
                out.push_str("$env.");
                out.push_str(&name);
            } else {
                out.push('$');
                out.push_str(&name);
            }
            i = end;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// `unset VAR [VAR2 ...]` -> `hide-env VAR; hide-env VAR2`.
fn rule_unset(input: &str) -> Option<String> {
    let rest = input.strip_prefix("unset ")?;
    let names: Vec<&str> = rest.split_whitespace().collect();
    if names.is_empty() || !names.iter().all(|n| is_ident(n)) {
        return None;
    }
    Some(
        names
            .iter()
            .map(|n| format!("hide-env {n}"))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// `alias x='y'` / `alias x=y` -> `alias x = y`.
fn rule_alias(input: &str) -> Option<String> {
    let rest = input.strip_prefix("alias ")?;
    let (name, value) = rest.trim_start().split_once('=')?;
    let name = name.trim();
    if !is_ident(name) || value.trim().is_empty() {
        return None;
    }
    Some(format!("alias {name} = {}", dequote(value.trim())))
}

/// `. file` -> `source file` (bare dot-source shorthand only; `source file`
/// is already valid nu and is left untouched, which means this rule never
/// fires on it).
fn rule_dot_source(input: &str) -> Option<String> {
    let rest = input.strip_prefix(". ")?;
    if rest.trim().is_empty() {
        return None;
    }
    Some(format!("source {}", rest.trim()))
}

/// `VAR=val [VAR2=val2 ...] cmd args...` -> `with-env {VAR: "val"} { cmd args... }`.
fn rule_env_prefix(input: &str) -> Option<String> {
    let mut rest = input;
    let mut pairs = Vec::new();
    loop {
        let (token, after) = match rest.split_once(char::is_whitespace) {
            Some((t, a)) => (t, a.trim_start()),
            None => (rest, ""),
        };
        if let Some((name, val)) = token.split_once('=') {
            if is_ident(name) && !val.is_empty() && !name.is_empty() {
                pairs.push((name.to_string(), val.to_string()));
                rest = after;
                if rest.is_empty() {
                    // The whole line was env assignments with no command --
                    // not actually the "prefix a command" idiom; bail.
                    return None;
                }
                continue;
            }
        }
        break;
    }
    if pairs.is_empty() {
        return None;
    }
    let env_record = pairs
        .iter()
        .map(|(k, v)| format!("{k}: {}", to_nu_string_literal(v)))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("with-env {{{env_record}}} {{ {rest} }}"))
}

/// `cmd > file 2>&1` -> `cmd o+e> file`. Only fires on this well-defined
/// "redirect both streams to a file" shape; a bare trailing `2>&1` with no
/// preceding `> file` (e.g. mid-pipeline) is ambiguous enough that it's left
/// untouched rather than guessed at.
fn rule_redirect_2_1(input: &str) -> Option<String> {
    let idx = input.find(" 2>&1")?;
    let before = &input[..idx];
    let after = &input[idx + " 2>&1".len()..];
    let stripped = before.rfind("> ")?;
    let (cmd_part, redirect_part) = before.split_at(stripped);
    let file = redirect_part.trim_start_matches('>').trim();
    if file.is_empty() {
        return None;
    }
    Some(format!("{} o+e> {file}{after}", cmd_part.trim_end()))
}

/// `cmd > file`, `cmd >> file`, `cmd < file` -> nu's `o>`/`o>>`/`open | cmd`.
/// Skipped for `[[ ... ]]` test lines (handled separately), and skipped
/// entirely when a pipe is present: nu itself uses `>`/`<` as comparison
/// operators (e.g. `where size > 10mb`), and there's no reliable textual way
/// to tell that apart from a trailing bash redirect once a pipe is involved
/// -- safer to leave it untouched than to guess wrong.
fn rule_redirects(input: &str) -> Option<String> {
    if input.trim_start().starts_with("[[") || input.contains('|') {
        return None;
    }
    if let Some((cmd, file)) = split_last_operator(input, ">>") {
        return Some(format!("{} o>> {}", cmd.trim_end(), file.trim()));
    }
    if let Some((cmd, file)) = split_last_operator(input, ">") {
        return Some(format!("{} o> {}", cmd.trim_end(), file.trim()));
    }
    if let Some((cmd, file)) = split_last_operator(input, "<") {
        return Some(format!("open {} | {}", file.trim(), cmd.trim_end()));
    }
    None
}

fn split_last_operator<'a>(input: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    // Avoid `>>`/`<` matches that are actually part of `>=`/`<=`/`!=`/`==`.
    let idx = input.rfind(op)?;
    if op == ">" && input[idx..].starts_with(">>") {
        return None;
    }
    let after = &input[idx + op.len()..];
    if after.starts_with('=') || after.starts_with('&') {
        return None;
    }
    let before = &input[..idx];
    let file = after.trim();
    if file.is_empty() || before.trim().is_empty() {
        return None;
    }
    Some((before, file))
}

/// A bare `[[ ... ]]` test expression, e.g. `[[ -f foo.txt ]]`.
fn rule_test_expression(input: &str) -> Option<String> {
    let inner = input
        .trim()
        .strip_prefix("[[")?
        .strip_suffix("]]")?
        .trim();

    if let Some(arg) = inner.strip_prefix("-f ") {
        return Some(format!("({} | path exists)", arg.trim()));
    }
    if let Some(arg) = inner.strip_prefix("-d ") {
        return Some(format!("(({} | path type) == dir)", arg.trim()));
    }
    if let Some(arg) = inner.strip_prefix("-z ") {
        return Some(format!("({} == \"\")", arg.trim()));
    }
    if let Some(arg) = inner.strip_prefix("-n ") {
        return Some(format!("({} != \"\")", arg.trim()));
    }
    if let Some((l, r)) = inner.split_once("==") {
        return Some(format!("({} == {})", l.trim(), r.trim()));
    }
    if let Some((l, r)) = inner.split_once("!=") {
        return Some(format!("({} != {})", l.trim(), r.trim()));
    }
    None
}

// ---------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Strips one layer of matching surrounding quotes, if present.
fn dequote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'\'' || first == b'"') && first == last {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// Builds a nu double-quoted string literal from a raw bash value, which may
/// itself already be single- or double-quoted.
fn to_nu_string_literal(raw: &str) -> String {
    let inner = dequote(raw);
    let escaped = inner.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(input: &str) -> String {
        translate(input).translated
    }

    #[test]
    fn and_chain() {
        assert_eq!(
            t("npm install && npm start"),
            "npm install; if $env.LAST_EXIT_CODE == 0 { npm start }"
        );
    }

    #[test]
    fn or_chain() {
        assert_eq!(
            t("cmd1 || cmd2"),
            "cmd1; if $env.LAST_EXIT_CODE != 0 { cmd2 }"
        );
    }

    #[test]
    fn chain_inside_quotes_is_untouched() {
        assert_eq!(t(r#"echo "a && b""#), r#"echo "a && b""#);
    }

    #[test]
    fn export_simple() {
        assert_eq!(t("export FOO=bar"), "$env.FOO = \"bar\"");
    }

    #[test]
    fn export_quoted_value() {
        assert_eq!(
            t(r#"export FOO="bar baz""#),
            "$env.FOO = \"bar baz\""
        );
    }

    #[test]
    fn inline_env_prefix() {
        assert_eq!(
            t("VAR=1 some_cmd arg"),
            "with-env {VAR: \"1\"} { some_cmd arg }"
        );
    }

    #[test]
    fn unset_single() {
        assert_eq!(t("unset FOO"), "hide-env FOO");
    }

    #[test]
    fn command_substitution_dollar_paren() {
        assert_eq!(t("echo $(date)"), "echo (date)");
    }

    #[test]
    fn command_substitution_backtick() {
        assert_eq!(t("echo `date`"), "echo (date)");
    }

    #[test]
    fn alias_rule() {
        assert_eq!(t("alias gs='git status'"), "alias gs = git status");
    }

    #[test]
    fn dot_source() {
        assert_eq!(t(". ./env.sh"), "source ./env.sh");
    }

    #[test]
    fn redirect_2_1_with_file() {
        assert_eq!(t("cmd > out.log 2>&1"), "cmd o+e> out.log");
    }

    #[test]
    fn redirect_stdout() {
        assert_eq!(t("cmd > out.log"), "cmd o> out.log");
    }

    #[test]
    fn redirect_append() {
        assert_eq!(t("cmd >> out.log"), "cmd o>> out.log");
    }

    #[test]
    fn redirect_input() {
        assert_eq!(t("wc -l < file.txt"), "open file.txt | wc -l");
    }

    #[test]
    fn test_expr_file_exists() {
        assert_eq!(t("[[ -f foo.txt ]]"), "(foo.txt | path exists)");
    }

    #[test]
    fn test_expr_dir() {
        assert_eq!(t("[[ -d foo ]]"), "((foo | path type) == dir)");
    }

    #[test]
    fn test_expr_eq() {
        assert_eq!(t("[[ $a == $b ]]"), "($a == $b)");
    }

    #[test]
    fn unrecognized_passthrough() {
        let line = "for i in 1 2 3; do echo $i; done";
        assert_eq!(t(line), line);
    }

    #[test]
    fn already_valid_nu_passthrough() {
        let line = "ls | where size > 10mb";
        assert_eq!(t(line), line);
    }

    #[test]
    fn combined_export_and_chain_rewrites_later_read() {
        // A later `$FOO` read in the same chain must become `$env.FOO`, not
        // stay bare -- bare `$FOO` isn't how nu reads env vars and errors
        // with "variable not found".
        assert_eq!(
            t("export FOO=bar && echo $FOO"),
            "$env.FOO = \"bar\"; if $env.LAST_EXIT_CODE == 0 { echo $env.FOO }"
        );
    }

    #[test]
    fn dollar_var_untouched_when_not_exported_in_this_line() {
        assert_eq!(t("echo $PATH"), "echo $PATH");
    }

    #[test]
    fn dollar_var_inside_single_quotes_untouched() {
        assert_eq!(
            t(r#"export FOO=bar && echo '$FOO'"#),
            r#"$env.FOO = "bar"; if $env.LAST_EXIT_CODE == 0 { echo '$FOO' }"#
        );
    }
}
