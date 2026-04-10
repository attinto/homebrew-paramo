use crate::paths;
use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};

const MAX_DAYS: usize = 60;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DayAttempts {
    pub date: NaiveDate,
    pub initiated: u32,
    pub completed: u32,
    pub resisted: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AttemptsLog {
    pub days: Vec<DayAttempts>,
}

pub fn load() -> Result<AttemptsLog> {
    let path = paths::user_attempts_file();
    if !path.exists() {
        return Ok(AttemptsLog::default());
    }

    let content = std::fs::read_to_string(path)?;
    let mut log: AttemptsLog = serde_json::from_str(&content)?;
    normalize(&mut log);
    Ok(log)
}

pub fn save(log: &AttemptsLog) -> Result<()> {
    let path = paths::user_attempts_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut normalized = log.clone();
    normalize(&mut normalized);

    let content = serde_json::to_string_pretty(&normalized)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn record_initiated() -> Result<()> {
    update_today(|day| {
        day.initiated += 1;
        day.resisted = day.initiated.saturating_sub(day.completed);
    })
}

pub fn record_completed() -> Result<()> {
    update_today(|day| {
        day.completed += 1;
        day.resisted = day.initiated.saturating_sub(day.completed);
    })
}

pub fn today() -> Result<DayAttempts> {
    let today = Local::now().date_naive();
    let log = load()?;
    Ok(log
        .days
        .into_iter()
        .find(|day| day.date == today)
        .unwrap_or(DayAttempts {
            date: today,
            initiated: 0,
            completed: 0,
            resisted: 0,
        }))
}

pub fn last_n_days(n: usize) -> Result<Vec<DayAttempts>> {
    if n == 0 {
        return Ok(Vec::new());
    }

    let today = Local::now().date_naive();
    let start = today - Duration::days((n.saturating_sub(1)) as i64);
    let log = load()?;
    let mut days = Vec::with_capacity(n);

    for offset in 0..n {
        let date = start + Duration::days(offset as i64);
        let day = log
            .days
            .iter()
            .find(|entry| entry.date == date)
            .cloned()
            .unwrap_or(DayAttempts {
                date,
                initiated: 0,
                completed: 0,
                resisted: 0,
            });
        days.push(day);
    }

    Ok(days)
}

fn update_today(update: impl FnOnce(&mut DayAttempts)) -> Result<()> {
    let today = Local::now().date_naive();
    let mut log = load()?;

    if let Some(day) = log.days.iter_mut().find(|day| day.date == today) {
        update(day);
    } else {
        let mut day = DayAttempts {
            date: today,
            initiated: 0,
            completed: 0,
            resisted: 0,
        };
        update(&mut day);
        log.days.push(day);
    }

    save(&log)
}

fn normalize(log: &mut AttemptsLog) {
    log.days.sort_by_key(|day| day.date);
    for day in &mut log.days {
        day.resisted = day.initiated.saturating_sub(day.completed);
    }
    if log.days.len() > MAX_DAYS {
        let start = log.days.len() - MAX_DAYS;
        log.days.drain(0..start);
    }
}
