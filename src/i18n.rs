use chrono::Weekday;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use toml::Table;

// Los archivos TOML se embeben en el binario en tiempo de compilación.
// Se parsean una sola vez al primer uso gracias a OnceLock.
static TRANSLATIONS: OnceLock<[Table; 2]> = OnceLock::new();

fn translations() -> &'static [Table; 2] {
    TRANSLATIONS.get_or_init(|| {
        let es: Table = include_str!("../locales/es.toml")
            .parse()
            .expect("locales/es.toml debe ser TOML válido");
        let en: Table = include_str!("../locales/en.toml")
            .parse()
            .expect("locales/en.toml debe ser TOML válido");
        [es, en]
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Es,
    En,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::Es => "es",
            Self::En => "en",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Self::Es => "Español",
            Self::En => "English",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "es" | "spa" | "spanish" | "espanol" | "español" => Some(Self::Es),
            "en" | "eng" | "english" => Some(Self::En),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct I18n {
    language: Language,
}

impl I18n {
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    pub fn language(self) -> Language {
        self.language
    }

    // Devuelve la cadena estática para la clave dada en el idioma activo.
    // Si la clave no existe, devuelve "???" — esto no debería ocurrir nunca
    // siempre que los archivos locales estén completos.
    fn t(self, key: &str) -> &'static str {
        let table = match self.language {
            Language::Es => &translations()[0],
            Language::En => &translations()[1],
        };
        table
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("???")
    }

    // Sustituye los marcadores {0}, {1}, {2} de una plantilla de traducción.
    fn format(self, key: &str, args: &[&str]) -> String {
        let mut result = self.t(key).to_string();
        for (i, arg) in args.iter().enumerate() {
            result = result.replace(&format!("{{{}}}", i), arg);
        }
        result
    }

    // --- Cadenas sin parámetros ---

    pub fn blocked_label(self) -> &'static str { self.t("blocked_label") }
    pub fn unblocked_label(self) -> &'static str { self.t("unblocked_label") }
    pub fn schedule_active_label(self) -> &'static str { self.t("schedule_active_label") }
    pub fn schedule_inactive_label(self) -> &'static str { self.t("schedule_inactive_label") }
    pub fn next_change_label(self) -> &'static str { self.t("next_change_label") }
    pub fn relative_time_label(self) -> &'static str { self.t("relative_time_label") }
    pub fn configured_sites_label(self) -> &'static str { self.t("configured_sites_label") }
    pub fn status_title(self) -> &'static str { self.t("status_title") }
    pub fn subtitle(self) -> &'static str { self.t("subtitle") }
    pub fn requires_root(self) -> &'static str { self.t("requires_root") }
    pub fn blocked_now(self) -> &'static str { self.t("blocked_now") }
    pub fn unblocked_now(self) -> &'static str { self.t("unblocked_now") }
    pub fn unblock_cancelled(self) -> &'static str { self.t("unblock_cancelled") }
    pub fn install_started(self) -> &'static str { self.t("install_started") }
    pub fn install_completed(self) -> &'static str { self.t("install_completed") }
    pub fn uninstall_started(self) -> &'static str { self.t("uninstall_started") }
    pub fn uninstall_completed(self) -> &'static str { self.t("uninstall_completed") }
    pub fn install_note(self) -> &'static str { self.t("install_note") }
    pub fn home_tab(self) -> &'static str { self.t("home_tab") }
    pub fn sites_tab(self) -> &'static str { self.t("sites_tab") }
    pub fn schedule_tab(self) -> &'static str { self.t("schedule_tab") }
    pub fn settings_tab(self) -> &'static str { self.t("settings_tab") }
    pub fn diagnostics_tab(self) -> &'static str { self.t("diagnostics_tab") }
    pub fn wall_tab(self) -> &'static str { self.t("wall_tab") }
    pub fn exit_tab(self) -> &'static str { self.t("exit_tab") }
    pub fn doctor_title(self) -> &'static str { self.t("doctor_title") }
    pub fn diagnostics_refresh(self) -> &'static str { self.t("diagnostics_refresh") }
    pub fn ok(self) -> &'static str { self.t("ok_label") }
    pub fn warning(self) -> &'static str { self.t("warning_label") }
    pub fn error(self) -> &'static str { self.t("error_label") }
    pub fn home_actions(self) -> &'static str { self.t("home_actions") }
    pub fn home_action_block(self) -> &'static str { self.t("home_action_block") }
    pub fn home_action_unblock(self) -> &'static str { self.t("home_action_unblock") }
    pub fn home_action_refresh(self) -> &'static str { self.t("home_action_refresh") }
    pub fn home_action_exit(self) -> &'static str { self.t("home_action_exit") }
    pub fn site_empty(self) -> &'static str { self.t("site_empty") }
    pub fn add_site_prompt(self) -> &'static str { self.t("add_site_prompt") }
    pub fn selected_label(self) -> &'static str { self.t("selected_label") }
    pub fn site_add_action(self) -> &'static str { self.t("site_add_action") }
    pub fn site_remove_action(self) -> &'static str { self.t("site_remove_action") }
    pub fn site_move_selection(self) -> &'static str { self.t("site_move_selection") }
    pub fn manage_label(self) -> &'static str { self.t("manage_label") }
    pub fn schedule_controls(self) -> &'static str { self.t("schedule_controls") }
    pub fn weekends_label(self) -> &'static str { self.t("weekends_label") }
    pub fn weekends_on(self) -> &'static str { self.t("weekends_on") }
    pub fn weekends_off(self) -> &'static str { self.t("weekends_off") }
    pub fn start_label(self) -> &'static str { self.t("start_label") }
    pub fn end_label(self) -> &'static str { self.t("end_label") }
    pub fn controls_label(self) -> &'static str { self.t("controls_label") }
    pub fn language_label(self) -> &'static str { self.t("language_label") }
    pub fn language_change_hint(self) -> &'static str { self.t("language_change_hint") }
    pub fn exit_screen_title(self) -> &'static str { self.t("exit_screen_title") }
    pub fn exit_screen_body(self) -> &'static str { self.t("exit_screen_body") }
    pub fn exit_screen_hint(self) -> &'static str { self.t("exit_screen_hint") }
    pub fn header_nav_hint(self) -> &'static str { self.t("header_nav_hint") }
    pub fn header_confirm_hint(self) -> &'static str { self.t("header_confirm_hint") }
    pub fn header_quit_hint(self) -> &'static str { self.t("header_quit_hint") }
    pub fn tui_hint(self) -> &'static str { self.t("tui_hint") }
    pub fn wall_title(self) -> &'static str { self.t("wall_title") }
    pub fn wall_empty(self) -> &'static str { self.t("wall_empty") }
    pub fn countdown_title(self) -> &'static str { self.t("countdown_title") }
    pub fn countdown_subtitle(self) -> &'static str { self.t("countdown_subtitle") }
    pub fn countdown_hint(self) -> &'static str { self.t("countdown_hint") }
    pub fn reason_prompt_title(self) -> &'static str { self.t("reason_prompt_title") }
    pub fn reason_prompt_hint(self) -> &'static str { self.t("reason_prompt_hint") }
    pub fn reason_required(self) -> &'static str { self.t("reason_required") }
    pub fn reason_required_label(self) -> &'static str { self.t("reason_required_label") }
    pub fn final_countdown_title(self) -> &'static str { self.t("final_countdown_title") }
    pub fn final_countdown_reason_label(self) -> &'static str { self.t("final_countdown_reason_label") }
    pub fn breath_in(self) -> &'static str { self.t("breath_in") }
    pub fn breath_out(self) -> &'static str { self.t("breath_out") }
    pub fn input_label(self) -> &'static str { self.t("input_label") }

    // --- Días de la semana ---

    pub fn weekday(self, weekday: Weekday) -> &'static str {
        let key = match weekday {
            Weekday::Mon => "weekday_mon",
            Weekday::Tue => "weekday_tue",
            Weekday::Wed => "weekday_wed",
            Weekday::Thu => "weekday_thu",
            Weekday::Fri => "weekday_fri",
            Weekday::Sat => "weekday_sat",
            Weekday::Sun => "weekday_sun",
        };
        self.t(key)
    }

    // --- Cadenas con parámetros ---

    pub fn language_updated(self, language: Language) -> String {
        self.format("language_updated", &[language.native_name()])
    }

    pub fn current_language(self, language: Language) -> String {
        self.format("current_language", &[language.native_name(), language.code()])
    }

    pub fn site_added(self, site: &str) -> String {
        self.format("site_added", &[site])
    }

    pub fn site_removed(self, site: &str) -> String {
        self.format("site_removed", &[site])
    }

    pub fn site_already_present(self, site: &str) -> String {
        self.format("site_already_present", &[site])
    }

    pub fn site_not_found(self, site: &str) -> String {
        self.format("site_not_found", &[site])
    }

    pub fn unsupported_language(self, value: &str) -> String {
        self.format("unsupported_language", &[value])
    }

    pub fn schedule_updated(self, start: u8, end: u8, weekends: bool) -> String {
        let weekends_str = if weekends { self.weekends_on() } else { self.weekends_off() };
        self.format(
            "schedule_updated",
            &[&format!("{:02}", start), &format!("{:02}", end), weekends_str],
        )
    }

    pub fn schedule_summary(self, start: u8, end: u8, weekends: bool) -> String {
        let weekends_str = if weekends { self.weekends_on() } else { self.weekends_off() };
        self.format(
            "schedule_summary",
            &[&format!("{:02}", start), &format!("{:02}", end), weekends_str],
        )
    }
}
