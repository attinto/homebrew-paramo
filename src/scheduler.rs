use crate::config::ScheduleConfig;
use chrono::{DateTime, Datelike, Local, Timelike, Weekday};

pub fn is_block_time(config: &ScheduleConfig, now: &DateTime<Local>) -> bool {
    if !config.block_weekends {
        let weekday = now.weekday();
        if weekday == Weekday::Sat || weekday == Weekday::Sun {
            return false;
        }
    }

    let hour = now.hour() as u8;

    if config.block_start < config.block_end {
        // Normal schedule (e.g., 09:00 - 18:00)
        hour >= config.block_start && hour < config.block_end
    } else {
        // Overnight schedule (e.g., 22:00 - 06:00)
        hour >= config.block_start || hour < config.block_end
    }
}

pub fn next_transition(config: &ScheduleConfig, now: &DateTime<Local>) -> Option<DateTime<Local>> {
    let currently_blocking = is_block_time(config, now);

    if currently_blocking {
        // We're currently blocking, next transition is at block_end
        let mut next = now
            .with_hour(config.block_end as u32)
            .and_then(|dt| dt.with_minute(0))
            .and_then(|dt| dt.with_second(0))?;

        // If the end time is on the next day, it will naturally be in the past today if start > end
        if next <= *now {
            next = next + chrono::Duration::days(1);
        }

        Some(next)
    } else {
        // Not blocking, next transition is at block_start
        let mut next = now
            .with_hour(config.block_start as u32)
            .and_then(|dt| dt.with_minute(0))
            .and_then(|dt| dt.with_second(0))?;

        if next <= *now {
            next = next + chrono::Duration::days(1);
        }

        // If block_weekends is false, skip weekends
        if !config.block_weekends {
            while next.weekday() == Weekday::Sat || next.weekday() == Weekday::Sun {
                next = next + chrono::Duration::days(1);
            }
        }

        Some(next)
    }
}

pub fn format_duration_until(until: &DateTime<Local>, now: &DateTime<Local>) -> String {
    let duration = until.signed_duration_since(*now);
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_is_block_time_within_hours() {
        let config = ScheduleConfig {
            block_start: 9,
            block_end: 18,
            block_weekends: false,
        };

        // Monday 10:00 - should block
        let monday_10 = Local.with_ymd_and_hms(2024, 3, 18, 10, 0, 0).unwrap();
        assert!(is_block_time(&config, &monday_10));

        // Monday 8:59 - should not block
        let monday_859 = Local.with_ymd_and_hms(2024, 3, 18, 8, 59, 0).unwrap();
        assert!(!is_block_time(&config, &monday_859));

        // Monday 18:00 - should not block (end is exclusive)
        let monday_1800 = Local.with_ymd_and_hms(2024, 3, 18, 18, 0, 0).unwrap();
        assert!(!is_block_time(&config, &monday_1800));
    }

    #[test]
    fn test_is_block_time_weekends() {
        let config = ScheduleConfig {
            block_start: 9,
            block_end: 18,
            block_weekends: false,
        };

        // Saturday 10:00 - should not block
        let saturday_10 = Local.with_ymd_and_hms(2024, 3, 16, 10, 0, 0).unwrap();
        assert!(!is_block_time(&config, &saturday_10));

        // With block_weekends = true, should block
        let config_weekends = ScheduleConfig {
            block_start: 9,
            block_end: 18,
            block_weekends: true,
        };
        assert!(is_block_time(&config_weekends, &saturday_10));
    }
}
