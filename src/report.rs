use crate::journal;
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};

pub fn weekly_report() -> String {
    let entries = journal::load().unwrap_or_default();
    let today = Local::now().date_naive();

    let days: Vec<NaiveDate> = (0..7).rev().map(|i| today - Duration::days(i)).collect();

    let counts: Vec<usize> = days
        .iter()
        .map(|day| {
            entries
                .iter()
                .filter(|e| e.timestamp.date_naive() == *day)
                .count()
        })
        .collect();

    let max_count = counts.iter().copied().max().unwrap_or(0).max(1);
    let bar_width: usize = 20;

    let day_names_es = ["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"];

    let mut out = String::new();

    out.push_str("╔══════════════════════════════════════════════╗\n");
    out.push_str("║        PARAMO · Reporte Semanal              ║\n");
    out.push_str("╚══════════════════════════════════════════════╝\n\n");

    for (i, (day, &count)) in days.iter().zip(counts.iter()).enumerate() {
        let day_name = day_names_es[weekday_index(day.weekday())];
        let date_str = day.format("%d/%m").to_string();
        let is_today = i == 6;

        let bar = if count == 0 {
            format!("{}", "·".repeat(bar_width))
        } else {
            let filled = ((count * bar_width) / max_count).max(1);
            let empty = bar_width - filled;
            format!("{}{}", "█".repeat(filled), "░".repeat(empty))
        };

        let suffix = if is_today {
            " ← hoy".to_string()
        } else if count == 0 {
            " ★".to_string()
        } else {
            String::new()
        };

        out.push_str(&format!(
            "  {} {}  │ {} │  {} desbloq{}\n",
            day_name, date_str, bar, count, suffix
        ));
    }

    let total: usize = counts.iter().sum();
    let clean_days = counts.iter().filter(|&&c| c == 0).count();
    let worst = counts.iter().copied().max().unwrap_or(0);
    let streak = counts.iter().rev().take_while(|&&c| c == 0).count();

    out.push_str("\n  ──────────────────────────────────────────────\n");
    out.push_str(&format!("  Desbloq. esta semana:   {}\n", total));
    out.push_str(&format!("  Días sin romper:         {} / 7\n", clean_days));
    out.push_str(&format!("  Peor día:                {} desbloq.\n", worst));
    out.push_str(&format!("  Racha actual:            {} días\n", streak));

    out.push('\n');
    out.push_str(match total {
        0 => "  ¡Semana perfecta! El colibrí está orgulloso de ti.\n",
        1..=3 => "  Muy bien. Casi impecable.\n",
        4..=10 => "  Hay margen de mejora. Tú puedes.\n",
        _ => "  Semana dura. ¿Has pensado en el Modo Monje?\n",
    });

    out
}

fn weekday_index(w: Weekday) -> usize {
    match w {
        Weekday::Sun => 0,
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
    }
}
