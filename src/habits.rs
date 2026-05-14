use crate::paths;
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HabitFrequency {
    Daily,
    Weekly,
    Monthly,
    Weekdays,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Habit {
    pub id: String,
    pub name: String,
    pub frequency: HabitFrequency,
    pub created_at: String,
    pub completions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HabitsLog {
    pub habits: Vec<Habit>,
}

pub fn load() -> Result<HabitsLog> {
    let path = paths::user_config_dir().join("habits.json");
    if !path.exists() {
        return Ok(HabitsLog::default());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save(log: &HabitsLog) -> Result<()> {
    let path = paths::user_config_dir().join("habits.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(log)?)?;
    Ok(())
}

pub fn new_habit(name: String, frequency: HabitFrequency) -> Habit {
    let now = Local::now();
    Habit {
        id: now.format("%Y%m%dT%H%M%S").to_string(),
        name,
        frequency,
        created_at: now.format("%Y-%m-%d").to_string(),
        completions: Vec::new(),
    }
}

// Returns the canonical start-of-period date for a given date and frequency.
// Comparing two period_start values tells you if they belong to the same period.
fn period_start(frequency: &HabitFrequency, date: NaiveDate) -> NaiveDate {
    match frequency {
        HabitFrequency::Daily | HabitFrequency::Weekdays => date,
        HabitFrequency::Weekly => {
            date - Duration::days(date.weekday().num_days_from_monday() as i64)
        }
        HabitFrequency::Monthly => {
            NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date)
        }
    }
}

// Returns the period_start of the period immediately before the one containing `date`.
fn prev_period(frequency: &HabitFrequency, date: NaiveDate) -> NaiveDate {
    let start = period_start(frequency, date);
    match frequency {
        HabitFrequency::Daily | HabitFrequency::Weekdays => start - Duration::days(1),
        HabitFrequency::Weekly => start - Duration::weeks(1),
        HabitFrequency::Monthly => {
            let (year, month) = if start.month() == 1 {
                (start.year() - 1, 12)
            } else {
                (start.year(), start.month() - 1)
            };
            NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(start - Duration::days(28))
        }
    }
}

pub fn is_due_today(habit: &Habit) -> bool {
    match habit.frequency {
        HabitFrequency::Weekdays => {
            let wd = Local::now().date_naive().weekday();
            !matches!(wd, Weekday::Sat | Weekday::Sun)
        }
        _ => true,
    }
}

pub fn is_completed_in_period(habit: &Habit, date: NaiveDate) -> bool {
    let target = period_start(&habit.frequency, date);
    habit.completions.iter().any(|c| {
        NaiveDate::parse_from_str(c, "%Y-%m-%d")
            .map(|d| period_start(&habit.frequency, d) == target)
            .unwrap_or(false)
    })
}

pub fn toggle_completion(habit: &mut Habit) {
    let today = Local::now().date_naive();
    if is_completed_in_period(habit, today) {
        let target = period_start(&habit.frequency, today);
        habit.completions.retain(|c| {
            NaiveDate::parse_from_str(c, "%Y-%m-%d")
                .map(|d| period_start(&habit.frequency, d) != target)
                .unwrap_or(true)
        });
    } else {
        let today_str = today.format("%Y-%m-%d").to_string();
        habit.completions.push(today_str);
    }
}

pub fn current_streak(habit: &Habit) -> u32 {
    let today = Local::now().date_naive();
    let mut streak = 0u32;
    let mut cursor = today;

    for _ in 0..366 {
        // For Weekdays, skip weekend days without breaking the streak
        if matches!(habit.frequency, HabitFrequency::Weekdays)
            && matches!(cursor.weekday(), Weekday::Sat | Weekday::Sun)
        {
            cursor -= Duration::days(1);
            continue;
        }

        if is_completed_in_period(habit, cursor) {
            streak += 1;
        } else if cursor == today {
            // Today not yet done — don't penalise current streak
        } else {
            break;
        }

        cursor = prev_period(&habit.frequency, cursor);
    }

    streak
}

pub fn best_streak(habit: &Habit) -> u32 {
    if habit.completions.is_empty() {
        return 0;
    }

    // Collect unique period-start dates, sorted ascending
    let mut periods: Vec<NaiveDate> = habit
        .completions
        .iter()
        .filter_map(|c| NaiveDate::parse_from_str(c, "%Y-%m-%d").ok())
        .filter(|&d| {
            // Exclude Weekdays-frequency completions on weekend days
            !(matches!(habit.frequency, HabitFrequency::Weekdays)
                && matches!(d.weekday(), Weekday::Sat | Weekday::Sun))
        })
        .map(|d| period_start(&habit.frequency, d))
        .collect();

    periods.sort();
    periods.dedup();

    if periods.is_empty() {
        return 0;
    }

    let mut best = 1u32;
    let mut run = 1u32;

    for w in periods.windows(2) {
        let expected_next = {
            // The period after w[0] should be w[1] for a consecutive run
            let after = match habit.frequency {
                HabitFrequency::Daily | HabitFrequency::Weekdays => w[0] + Duration::days(1),
                HabitFrequency::Weekly => w[0] + Duration::weeks(1),
                HabitFrequency::Monthly => {
                    let (y, m) = if w[0].month() == 12 {
                        (w[0].year() + 1, 1)
                    } else {
                        (w[0].year(), w[0].month() + 1)
                    };
                    NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(w[0])
                }
            };
            period_start(&habit.frequency, after)
        };

        if matches!(habit.frequency, HabitFrequency::Weekdays) {
            // For weekdays, consecutive means the next non-weekend day
            let mut day = w[0] + Duration::days(1);
            while matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
                day += Duration::days(1);
            }
            if w[1] == day {
                run += 1;
            } else {
                run = 1;
            }
        } else if w[1] == expected_next {
            run += 1;
        } else {
            run = 1;
        }

        best = best.max(run);
    }

    best
}

pub fn completion_rate_last_n(habit: &Habit, n: u32) -> u8 {
    if n == 0 {
        return 0;
    }
    let today = Local::now().date_naive();
    let mut due = 0u32;
    let mut done = 0u32;

    for offset in 0..n {
        let date = today - Duration::days(offset as i64);
        if matches!(habit.frequency, HabitFrequency::Weekdays)
            && matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
        {
            continue;
        }
        due += 1;
        if is_completed_in_period(habit, date) {
            done += 1;
        }
    }

    if due == 0 {
        return 0;
    }
    ((done * 100) / due).min(100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn make_habit(freq: HabitFrequency, completions: &[&str]) -> Habit {
        Habit {
            id: "test".into(),
            name: "test".into(),
            frequency: freq,
            created_at: "2024-01-01".into(),
            completions: completions.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_period_start_weekly_thursday() {
        let thu = date("2024-01-11");
        let mon = date("2024-01-08");
        assert_eq!(period_start(&HabitFrequency::Weekly, thu), mon);
    }

    #[test]
    fn test_period_start_monthly() {
        let mid = date("2024-03-15");
        let first = date("2024-03-01");
        assert_eq!(period_start(&HabitFrequency::Monthly, mid), first);
    }

    #[test]
    fn test_period_start_daily_is_self() {
        let d = date("2024-06-20");
        assert_eq!(period_start(&HabitFrequency::Daily, d), d);
    }

    #[test]
    fn test_is_completed_weekly_same_week() {
        // Completed on Wed 2024-01-10 — should match any day in that ISO week
        let h = make_habit(HabitFrequency::Weekly, &["2024-01-10"]);
        assert!(is_completed_in_period(&h, date("2024-01-08"))); // Mon
        assert!(is_completed_in_period(&h, date("2024-01-14"))); // Sun
        assert!(!is_completed_in_period(&h, date("2024-01-15"))); // Next Mon
    }

    #[test]
    fn test_is_completed_monthly() {
        let h = make_habit(HabitFrequency::Monthly, &["2024-03-15"]);
        assert!(is_completed_in_period(&h, date("2024-03-01")));
        assert!(is_completed_in_period(&h, date("2024-03-31")));
        assert!(!is_completed_in_period(&h, date("2024-04-01")));
    }

    #[test]
    fn test_toggle_adds_then_removes() {
        let mut h = make_habit(HabitFrequency::Daily, &[]);
        let today = Local::now().date_naive();
        assert!(!is_completed_in_period(&h, today));
        toggle_completion(&mut h);
        assert!(is_completed_in_period(&h, today));
        assert_eq!(h.completions.len(), 1);
        toggle_completion(&mut h);
        assert!(!is_completed_in_period(&h, today));
        assert!(h.completions.is_empty());
    }

    #[test]
    fn test_best_streak_daily() {
        // 3 consecutive days
        let h = make_habit(
            HabitFrequency::Daily,
            &["2024-01-01", "2024-01-02", "2024-01-03"],
        );
        assert_eq!(best_streak(&h), 3);
    }

    #[test]
    fn test_best_streak_gap_resets() {
        // Gap on 2024-01-02 — two separate runs of 1
        let h = make_habit(HabitFrequency::Daily, &["2024-01-01", "2024-01-03"]);
        assert_eq!(best_streak(&h), 1);
    }

    #[test]
    fn test_completion_rate_all_done() {
        let today = Local::now().date_naive();
        let completions: Vec<String> = (0..7)
            .map(|i| (today - Duration::days(i)).format("%Y-%m-%d").to_string())
            .collect();
        let h = Habit {
            completions,
            ..make_habit(HabitFrequency::Daily, &[])
        };
        assert_eq!(completion_rate_last_n(&h, 7), 100);
    }


}
