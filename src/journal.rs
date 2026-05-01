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
    let resolved = if reason.is_empty() {
        "(sin motivo)"
    } else {
        reason
    };
    write_entry(resolved)?;
    if let Err(error) = streak::record_break(resolved) {
        eprintln!("Failed to record streak break: {error}");
    }
    Ok(())
}

// Eliminar un sitio también pasa por el muro, para que quede traza visible
// del motivo, pero NO rompe la racha: la racha sólo penaliza desbloquear
// dentro del horario.
pub fn append_site_removal(site: &str, reason: &str) -> std::io::Result<()> {
    let reason = if reason.is_empty() {
        "(sin motivo)"
    } else {
        reason
    };
    let line = format!("[Eliminado sitio {site}] {reason}");
    write_entry(&line)
}

fn write_entry(line: &str) -> std::io::Result<()> {
    let path = paths::user_journal_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let timestamp = Local::now().to_rfc3339();

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    writeln!(file, "{}|{}", timestamp, line)?;
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
