use crate::paths;
use crate::streak;
use chrono::{DateTime, Local};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub timestamp: DateTime<Local>,
    pub reason: String,
}

pub fn append(reason: &str) -> std::io::Result<()> {
    let path = paths::user_journal_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let timestamp = Local::now().to_rfc3339();
    let reason = if reason.is_empty() {
        "(sin motivo)"
    } else {
        reason
    };

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    writeln!(file, "{}|{}", timestamp, reason)?;
    if let Err(error) = streak::record_break(reason) {
        eprintln!("Failed to record streak break: {error}");
    }
    Ok(())
}

pub fn load() -> std::io::Result<Vec<JournalEntry>> {
    let path = paths::user_journal_file();
    if !path.exists() {
        return Ok(vec![]);
    }

    let file = std::fs::File::open(&path)?;
    let reader = BufReader::new(file);

    let mut entries: Vec<JournalEntry> = reader
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            let (ts, reason) = line.split_once('|')?;
            let timestamp = DateTime::parse_from_rfc3339(ts).ok()?.with_timezone(&Local);
            Some(JournalEntry {
                timestamp,
                reason: reason.to_string(),
            })
        })
        .collect();

    entries.reverse(); // más recientes primero
    Ok(entries)
}
