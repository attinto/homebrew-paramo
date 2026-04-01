use crate::paths;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreakState {
    pub current: u32,
    pub best: u32,
    pub last_clean: Option<NaiveDate>,
    pub last_break: Option<NaiveDate>,
    pub last_break_reason: Option<String>,
    pub total_breaks: u32,
}

pub fn load() -> Result<StreakState> {
    let path = paths::user_config_dir().join("streak.json");
    if !path.exists() {
        return Ok(StreakState::default());
    }

    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save(state: &StreakState) -> Result<()> {
    let path = paths::user_config_dir().join("streak.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

pub fn record_clean_day() -> Result<StreakState> {
    let today = Local::now().date_naive();
    let mut state = load()?;

    if state.last_clean != Some(today) {
        state.current += 1;
        state.best = state.best.max(state.current);
        state.last_clean = Some(today);
        save(&state)?;
    }

    Ok(state)
}

pub fn record_break(reason: &str) -> Result<StreakState> {
    let today = Local::now().date_naive();
    let mut state = load()?;

    state.current = 0;
    state.total_breaks += 1;
    state.last_break = Some(today);
    state.last_break_reason = Some(reason.to_string());
    save(&state)?;

    Ok(state)
}
