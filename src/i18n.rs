use chrono::Weekday;
use serde::{Deserialize, Serialize};

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

    pub fn blocked_label(self) -> &'static str {
        match self.language {
            Language::Es => "BLOQUEADO",
            Language::En => "BLOCKED",
        }
    }

    pub fn unblocked_label(self) -> &'static str {
        match self.language {
            Language::Es => "DESBLOQUEADO",
            Language::En => "UNBLOCKED",
        }
    }

    pub fn schedule_active_label(self) -> &'static str {
        match self.language {
            Language::Es => "Horario de bloqueo activo",
            Language::En => "Blocking schedule is active",
        }
    }

    pub fn schedule_inactive_label(self) -> &'static str {
        match self.language {
            Language::Es => "Fuera del horario de bloqueo",
            Language::En => "Outside the blocking schedule",
        }
    }

    pub fn next_change_label(self) -> &'static str {
        match self.language {
            Language::Es => "Próximo cambio",
            Language::En => "Next change",
        }
    }

    pub fn relative_time_label(self) -> &'static str {
        match self.language {
            Language::Es => "desde ahora",
            Language::En => "from now",
        }
    }

    pub fn configured_sites_label(self) -> &'static str {
        match self.language {
            Language::Es => "Sitios configurados",
            Language::En => "Configured sites",
        }
    }

    pub fn weekday(self, weekday: Weekday) -> &'static str {
        match self.language {
            Language::Es => match weekday {
                Weekday::Mon => "Lunes",
                Weekday::Tue => "Martes",
                Weekday::Wed => "Miércoles",
                Weekday::Thu => "Jueves",
                Weekday::Fri => "Viernes",
                Weekday::Sat => "Sábado",
                Weekday::Sun => "Domingo",
            },
            Language::En => match weekday {
                Weekday::Mon => "Monday",
                Weekday::Tue => "Tuesday",
                Weekday::Wed => "Wednesday",
                Weekday::Thu => "Thursday",
                Weekday::Fri => "Friday",
                Weekday::Sat => "Saturday",
                Weekday::Sun => "Sunday",
            },
        }
    }

    pub fn requires_root(self) -> &'static str {
        match self.language {
            Language::Es => "Esta acción requiere permisos de administrador. Ejecútala con sudo.",
            Language::En => "This action requires administrator privileges. Run it with sudo.",
        }
    }

    pub fn blocked_now(self) -> &'static str {
        match self.language {
            Language::Es => "Bloqueo manual aplicado.",
            Language::En => "Manual block applied.",
        }
    }

    pub fn unblocked_now(self) -> &'static str {
        match self.language {
            Language::Es => "Bloqueo manual retirado.",
            Language::En => "Manual block removed.",
        }
    }

    pub fn language_updated(self, language: Language) -> String {
        match self.language {
            Language::Es => format!("Idioma actualizado a {}.", language.native_name()),
            Language::En => format!("Language updated to {}.", language.native_name()),
        }
    }

    pub fn current_language(self, language: Language) -> String {
        match self.language {
            Language::Es => format!(
                "Idioma actual: {} ({})",
                language.native_name(),
                language.code()
            ),
            Language::En => format!(
                "Current language: {} ({})",
                language.native_name(),
                language.code()
            ),
        }
    }

    pub fn site_added(self, site: &str) -> String {
        match self.language {
            Language::Es => format!("Sitio añadido: {}", site),
            Language::En => format!("Site added: {}", site),
        }
    }

    pub fn site_removed(self, site: &str) -> String {
        match self.language {
            Language::Es => format!("Sitio eliminado: {}", site),
            Language::En => format!("Site removed: {}", site),
        }
    }

    pub fn site_already_present(self, site: &str) -> String {
        match self.language {
            Language::Es => format!("El sitio ya estaba en la lista: {}", site),
            Language::En => format!("Site was already in the list: {}", site),
        }
    }

    pub fn site_not_found(self, site: &str) -> String {
        match self.language {
            Language::Es => format!("El sitio no estaba en la lista: {}", site),
            Language::En => format!("Site was not in the list: {}", site),
        }
    }

    pub fn schedule_updated(self, start: u8, end: u8, weekends: bool) -> String {
        match self.language {
            Language::Es => format!(
                "Horario actualizado: {:02}:00 -> {:02}:00 | fines de semana: {}",
                start,
                end,
                self.on_off_spanish(weekends)
            ),
            Language::En => format!(
                "Schedule updated: {:02}:00 -> {:02}:00 | weekends: {}",
                start,
                end,
                self.on_off_english(weekends)
            ),
        }
    }

    pub fn schedule_summary(self, start: u8, end: u8, weekends: bool) -> String {
        match self.language {
            Language::Es => format!(
                "Horario: {:02}:00 -> {:02}:00 | fines de semana: {}",
                start,
                end,
                self.on_off_spanish(weekends)
            ),
            Language::En => format!(
                "Schedule: {:02}:00 -> {:02}:00 | weekends: {}",
                start,
                end,
                self.on_off_english(weekends)
            ),
        }
    }

    pub fn install_started(self) -> &'static str {
        match self.language {
            Language::Es => "Instalando PARAMO...",
            Language::En => "Installing PARAMO...",
        }
    }

    pub fn install_completed(self) -> &'static str {
        match self.language {
            Language::Es => "Instalación completada.",
            Language::En => "Installation completed.",
        }
    }

    pub fn uninstall_started(self) -> &'static str {
        match self.language {
            Language::Es => "Desinstalando PARAMO...",
            Language::En => "Uninstalling PARAMO...",
        }
    }

    pub fn uninstall_completed(self) -> &'static str {
        match self.language {
            Language::Es => "Desinstalación completada.",
            Language::En => "Uninstall completed.",
        }
    }

    pub fn doctor_title(self) -> &'static str {
        match self.language {
            Language::Es => "Diagnóstico",
            Language::En => "Diagnostics",
        }
    }

    pub fn home_tab(self) -> &'static str {
        match self.language {
            Language::Es => "Inicio",
            Language::En => "Home",
        }
    }

    pub fn sites_tab(self) -> &'static str {
        match self.language {
            Language::Es => "Sitios",
            Language::En => "Sites",
        }
    }

    pub fn schedule_tab(self) -> &'static str {
        match self.language {
            Language::Es => "Horario",
            Language::En => "Schedule",
        }
    }

    pub fn settings_tab(self) -> &'static str {
        match self.language {
            Language::Es => "Ajustes",
            Language::En => "Settings",
        }
    }

    pub fn diagnostics_tab(self) -> &'static str {
        match self.language {
            Language::Es => "Diagnóstico",
            Language::En => "Diagnostics",
        }
    }

    pub fn exit_tab(self) -> &'static str {
        match self.language {
            Language::Es => "Salir",
            Language::En => "Exit",
        }
    }

    pub fn diagnostics_refresh(self) -> &'static str {
        match self.language {
            Language::Es => "Pulsa g para relanzar el diagnóstico.",
            Language::En => "Press g to rerun diagnostics.",
        }
    }

    pub fn tui_hint(self) -> &'static str {
        match self.language {
            Language::Es => "Tab/cursor para navegar, a para añadir, d para borrar, b/u para bloquear, q para salir o pestaña Salir + Enter",
            Language::En => "Use Tab/arrows to navigate, a to add, d to delete, b/u to block, q to quit or Exit tab + Enter",
        }
    }

    pub fn add_site_prompt(self) -> &'static str {
        match self.language {
            Language::Es => "Añadir sitio: escribe un dominio y pulsa Enter",
            Language::En => "Add site: type a domain and press Enter",
        }
    }

    pub fn site_empty(self) -> &'static str {
        match self.language {
            Language::Es => "No hay sitios configurados.",
            Language::En => "No sites configured.",
        }
    }

    pub fn language_label(self) -> &'static str {
        match self.language {
            Language::Es => "Idioma",
            Language::En => "Language",
        }
    }

    pub fn ok(self) -> &'static str {
        match self.language {
            Language::Es => "OK",
            Language::En => "OK",
        }
    }

    pub fn warning(self) -> &'static str {
        match self.language {
            Language::Es => "AVISO",
            Language::En => "WARN",
        }
    }

    pub fn error(self) -> &'static str {
        match self.language {
            Language::Es => "ERROR",
            Language::En => "ERROR",
        }
    }

    pub fn home_actions(self) -> &'static str {
        match self.language {
            Language::Es => "Acciones rápidas",
            Language::En => "Quick actions",
        }
    }

    pub fn home_action_block(self) -> &'static str {
        match self.language {
            Language::Es => "b  Bloquear ahora",
            Language::En => "b  Block now",
        }
    }

    pub fn home_action_unblock(self) -> &'static str {
        match self.language {
            Language::Es => "u  Desbloquear ahora",
            Language::En => "u  Unblock now",
        }
    }

    pub fn home_action_refresh(self) -> &'static str {
        match self.language {
            Language::Es => "r  Recargar estado",
            Language::En => "r  Refresh status",
        }
    }

    pub fn home_action_exit(self) -> &'static str {
        match self.language {
            Language::Es => "Salir  Pestaña Salir + Enter o q",
            Language::En => "Exit  Exit tab + Enter or q",
        }
    }

    pub fn exit_screen_title(self) -> &'static str {
        match self.language {
            Language::Es => "Salir de PARAMO",
            Language::En => "Exit PARAMO",
        }
    }

    pub fn exit_screen_body(self) -> &'static str {
        match self.language {
            Language::Es => "Pulsa Enter para cerrar la TUI.",
            Language::En => "Press Enter to close the TUI.",
        }
    }

    pub fn exit_screen_hint(self) -> &'static str {
        match self.language {
            Language::Es => "También puedes salir en cualquier momento con q.",
            Language::En => "You can also quit at any time with q.",
        }
    }

    pub fn header_nav_hint(self) -> &'static str {
        match self.language {
            Language::Es => "Tab  Navegar",
            Language::En => "Tab  Navigate",
        }
    }

    pub fn header_confirm_hint(self) -> &'static str {
        match self.language {
            Language::Es => "Enter  Confirmar",
            Language::En => "Enter  Confirm",
        }
    }

    pub fn header_quit_hint(self) -> &'static str {
        match self.language {
            Language::Es => "q  Salir",
            Language::En => "q  Quit",
        }
    }

    pub fn schedule_controls(self) -> &'static str {
        match self.language {
            Language::Es => {
                "Cursores arriba/abajo seleccionan campo. Izquierda/derecha cambia valor."
            }
            Language::En => "Use up/down to select a field. Left/right changes the value.",
        }
    }

    pub fn weekends_label(self) -> &'static str {
        match self.language {
            Language::Es => "Fines de semana",
            Language::En => "Weekends",
        }
    }

    pub fn start_label(self) -> &'static str {
        match self.language {
            Language::Es => "Inicio",
            Language::En => "Start",
        }
    }

    pub fn end_label(self) -> &'static str {
        match self.language {
            Language::Es => "Fin",
            Language::En => "End",
        }
    }

    pub fn install_note(self) -> &'static str {
        match self.language {
            Language::Es => "Instalación del sistema: ejecuta `sudo paramo install` para crear `/etc/paramo` y registrar el daemon.",
            Language::En => "System install: run `sudo paramo install` to create `/etc/paramo` and register the daemon.",
        }
    }

    pub fn unsupported_language(self, value: &str) -> String {
        match self.language {
            Language::Es => format!("Idioma no soportado: {}", value),
            Language::En => format!("Unsupported language: {}", value),
        }
    }

    pub fn on_off_spanish(self, value: bool) -> &'static str {
        if value {
            "activados"
        } else {
            "desactivados"
        }
    }

    pub fn on_off_english(self, value: bool) -> &'static str {
        if value {
            "on"
        } else {
            "off"
        }
    }
}
