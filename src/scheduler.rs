use crate::config::ScheduleConfig;
use chrono::{DateTime, Datelike, Duration, Local, Timelike, Weekday};

pub fn is_block_time(config: &ScheduleConfig, now: &DateTime<Local>) -> bool {
    if !config.block_weekends {
        match now.weekday() {
            Weekday::Sat | Weekday::Sun => return false,
            _ => {}
        }
    }

    let hour = now.hour() as u8;

    if config.start < config.end {
        hour >= config.start && hour < config.end
    } else {
        hour >= config.start || hour < config.end
    }
}

pub fn next_transition(config: &ScheduleConfig, now: &DateTime<Local>) -> Option<DateTime<Local>> {
    let mut next = if is_block_time(config, now) {
        now.with_hour(config.end as u32)?
            .with_minute(0)?
            .with_second(0)?
    } else {
        now.with_hour(config.start as u32)?
            .with_minute(0)?
            .with_second(0)?
    };

    if next <= *now {
        next += Duration::days(1);
    }

    if !is_block_time(config, now) && !config.block_weekends {
        while matches!(next.weekday(), Weekday::Sat | Weekday::Sun) {
            next += Duration::days(1);
        }
    }

    Some(next)
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
            start: 9,
            end: 18,
            block_weekends: false,
        };

        let monday_10 = Local.with_ymd_and_hms(2024, 3, 18, 10, 0, 0).unwrap();
        assert!(is_block_time(&config, &monday_10));

        let monday_859 = Local.with_ymd_and_hms(2024, 3, 18, 8, 59, 0).unwrap();
        assert!(!is_block_time(&config, &monday_859));

        let monday_1800 = Local.with_ymd_and_hms(2024, 3, 18, 18, 0, 0).unwrap();
        assert!(!is_block_time(&config, &monday_1800));
    }

    #[test]
    fn test_is_block_time_weekends() {
        let config = ScheduleConfig {
            start: 9,
            end: 18,
            block_weekends: false,
        };

        let saturday_10 = Local.with_ymd_and_hms(2024, 3, 16, 10, 0, 0).unwrap();
        assert!(!is_block_time(&config, &saturday_10));

        let config_weekends = ScheduleConfig {
            start: 9,
            end: 18,
            block_weekends: true,
        };
        assert!(is_block_time(&config_weekends, &saturday_10));
    }
}
