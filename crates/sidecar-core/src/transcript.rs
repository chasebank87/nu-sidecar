use crate::xdg;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptEntry {
    pub ts: String,
    pub cmd: String,
    #[serde(default)]
    pub output: Option<String>,
    pub exit_code: i64,
}

pub fn session_file(pid: u32) -> PathBuf {
    xdg::sessions_dir().join(format!("{pid}.jsonl"))
}

/// Truncates `text` to `max_chars`, keeping the head and tail (most useful
/// for e.g. a build log where both the first error and the final summary
/// matter) and noting how much was cut from the middle.
pub fn cap_text(text: &str, max_chars: usize) -> String {
    let len = text.chars().count();
    if len <= max_chars {
        return text.to_string();
    }
    let half = max_chars / 2;
    let chars: Vec<char> = text.chars().collect();
    let head: String = chars[..half].iter().collect();
    let tail: String = chars[len - half..].iter().collect();
    let cut = len - (2 * half);
    format!("{head}\n...[{cut} chars truncated]...\n{tail}")
}

pub fn append_entry(
    pid: u32,
    mut entry: TranscriptEntry,
    max_chars_per_entry: usize,
) -> io::Result<()> {
    let dir = xdg::sessions_dir();
    fs::create_dir_all(&dir)?;
    if let Some(output) = &entry.output {
        entry.output = Some(cap_text(output, max_chars_per_entry));
    }
    let line = serde_json::to_string(&entry).unwrap_or_default();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(session_file(pid))?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// Reads the transcript for `pid`, most-recent-first, keeping at most
/// `max_entries` turns (or all of them if `None`) subject to a total
/// character budget across the whole assembled context.
pub fn read_context(
    pid: u32,
    max_entries: Option<usize>,
    max_total_chars: usize,
) -> io::Result<Vec<TranscriptEntry>> {
    let path = session_file(pid);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut all: Vec<TranscriptEntry> = reader
        .lines()
        .filter_map(Result::ok)
        .filter_map(|line| serde_json::from_str::<TranscriptEntry>(&line).ok())
        .collect();

    all.reverse(); // most recent first

    if let Some(n) = max_entries {
        all.truncate(n);
    }

    let mut budget = max_total_chars;
    let mut selected = Vec::new();
    for entry in all {
        let entry_len = entry.cmd.len() + entry.output.as_deref().unwrap_or("").len();
        if entry_len > budget && !selected.is_empty() {
            break;
        }
        budget = budget.saturating_sub(entry_len);
        selected.push(entry);
        if budget == 0 {
            break;
        }
    }

    selected.reverse(); // back to chronological order
    Ok(selected)
}
