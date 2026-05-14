use crate::attempts::{self, DayAttempts};
use crate::blocker::{self, StatusSnapshot};
use crate::config::SystemConfig;
use crate::doctor::{self, Diagnostic};
use crate::i18n::I18n;
use crate::journal;
use crate::preferences::UserPreferences;
use crate::streak::{self, StreakState};
use anyhow::Result;
use ratatui::widgets::ListState;
use std::time::Instant;

// Acción que está pendiente detrás de un flujo de fricción.
// Cada variante implica el mismo proceso (30s + motivo + 60s) pero el
// mensaje y el efecto final cambian.
#[derive(Debug, Clone)]
pub(crate) enum FrictionAction {
    Unblock,
    RemoveSite(String),
}

#[derive(Debug)]
pub(crate) enum FrictionFlow {
    Countdown {
        action: FrictionAction,
        started: Instant,
    },
    ReasonPrompt {
        action: FrictionAction,
        value: String,
    },
    FinalCountdown {
        action: FrictionAction,
        started: Instant,
        reason: String,
    },
}

impl FrictionFlow {
    pub(crate) fn action(&self) -> &FrictionAction {
        match self {
            Self::Countdown { action, .. }
            | Self::ReasonPrompt { action, .. }
            | Self::FinalCountdown { action, .. } => action,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PendingFriction {
    pub(crate) action: FrictionAction,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TabId {
    Home,
    Sites,
    Schedule,
    Settings,
    Diagnostics,
    Attempts,
    Streak,
    Wall,
    Exit,
}

impl TabId {
    pub(crate) fn all() -> [Self; 9] {
        [
            Self::Home,
            Self::Sites,
            Self::Schedule,
            Self::Settings,
            Self::Diagnostics,
            Self::Attempts,
            Self::Streak,
            Self::Wall,
            Self::Exit,
        ]
    }

    pub(crate) fn index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Sites => 1,
            Self::Schedule => 2,
            Self::Settings => 3,
            Self::Diagnostics => 4,
            Self::Attempts => 5,
            Self::Streak => 6,
            Self::Wall => 7,
            Self::Exit => 8,
        }
    }

    pub(crate) fn from_index(index: usize) -> Self {
        Self::all()[index % Self::all().len()]
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PromptState {
    pub(crate) title: String,
    pub(crate) value: String,
}

#[derive(Debug)]
pub(crate) struct Dashboard {
    pub(crate) config: SystemConfig,
    pub(crate) prefs: UserPreferences,
    pub(crate) i18n: I18n,
    pub(crate) status: StatusSnapshot,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) active_tab: TabId,
    pub(crate) sites_state: ListState,
    pub(crate) diagnostics_state: ListState,
    pub(crate) schedule_cursor: usize,
    pub(crate) flash_message: Option<String>,
    pub(crate) prompt: Option<PromptState>,
    pub(crate) friction_flow: Option<FrictionFlow>,
    pub(crate) pending_action: Option<PendingFriction>,
    pub(crate) streak: StreakState,
    pub(crate) attempts_today: DayAttempts,
    pub(crate) attempts_last_7_days: Vec<DayAttempts>,
    pub(crate) wall_entries: Vec<journal::JournalEntry>,
    pub(crate) wall_state: ListState,
}

impl Dashboard {
    pub(crate) fn new(config: SystemConfig, prefs: UserPreferences) -> Result<Self> {
        let i18n = I18n::new(prefs.language);
        let diagnostics = doctor::run(&config, i18n)?;
        let status = blocker::status_snapshot(&config)?;
        let mut sites_state = ListState::default();
        let mut diagnostics_state = ListState::default();

        if !config.sites.list.is_empty() {
            sites_state.select(Some(0));
        }
        if !diagnostics.is_empty() {
            diagnostics_state.select(Some(0));
        }

        Ok(Self {
            config,
            prefs,
            i18n,
            status,
            diagnostics,
            active_tab: TabId::Home,
            sites_state,
            diagnostics_state,
            schedule_cursor: 0,
            flash_message: None,
            prompt: None,
            friction_flow: None,
            pending_action: None,
            streak: streak::load().unwrap_or_default(),
            attempts_today: attempts::today().unwrap_or_default(),
            attempts_last_7_days: attempts::last_n_days(7).unwrap_or_default(),
            wall_entries: journal::load().unwrap_or_default(),
            wall_state: ListState::default(),
        })
    }

    pub(crate) fn set_flash(&mut self, message: impl Into<String>) {
        self.flash_message = Some(message.into());
    }

    pub(crate) fn next_tab(&mut self) {
        self.active_tab = TabId::from_index(self.active_tab.index() + 1);
    }

    pub(crate) fn previous_tab(&mut self) {
        let count = TabId::all().len();
        self.active_tab = TabId::from_index((self.active_tab.index() + count - 1) % count);
    }

    pub(crate) fn refresh_status(&mut self) -> Result<()> {
        self.status = blocker::status_snapshot(&self.config)?;
        self.streak = streak::load().unwrap_or_default();
        self.refresh_attempts();
        Ok(())
    }

    pub(crate) fn refresh_attempts(&mut self) {
        self.attempts_today = attempts::today().unwrap_or_default();
        self.attempts_last_7_days = attempts::last_n_days(7).unwrap_or_default();
    }

    pub(crate) fn refresh_diagnostics(&mut self) -> Result<()> {
        self.diagnostics = doctor::run(&self.config, self.i18n)?;
        if self.diagnostics.is_empty() {
            self.diagnostics_state.select(None);
        } else if self.diagnostics_state.selected().is_none() {
            self.diagnostics_state.select(Some(0));
        }
        Ok(())
    }

    pub(crate) fn select_next_site(&mut self) {
        if self.config.sites.list.is_empty() {
            self.sites_state.select(None);
            return;
        }
        let next = match self.sites_state.selected() {
            Some(index) => (index + 1) % self.config.sites.list.len(),
            None => 0,
        };
        self.sites_state.select(Some(next));
    }

    pub(crate) fn select_previous_site(&mut self) {
        if self.config.sites.list.is_empty() {
            self.sites_state.select(None);
            return;
        }
        let len = self.config.sites.list.len();
        let next = match self.sites_state.selected() {
            Some(index) => (index + len - 1) % len,
            None => 0,
        };
        self.sites_state.select(Some(next));
    }

    pub(crate) fn select_next_diagnostic(&mut self) {
        if self.diagnostics.is_empty() {
            self.diagnostics_state.select(None);
            return;
        }
        let next = match self.diagnostics_state.selected() {
            Some(index) => (index + 1) % self.diagnostics.len(),
            None => 0,
        };
        self.diagnostics_state.select(Some(next));
    }

    pub(crate) fn select_previous_diagnostic(&mut self) {
        if self.diagnostics.is_empty() {
            self.diagnostics_state.select(None);
            return;
        }
        let len = self.diagnostics.len();
        let next = match self.diagnostics_state.selected() {
            Some(index) => (index + len - 1) % len,
            None => 0,
        };
        self.diagnostics_state.select(Some(next));
    }

    // Avanza la máquina de estados de fricción en cada tick de render.
    // Countdown (30s) → ReasonPrompt → FinalCountdown (60s) → pending_action
    pub(crate) fn advance_friction_flow(&mut self) {
        let to_reason = match &self.friction_flow {
            Some(FrictionFlow::Countdown { started, .. }) => started.elapsed().as_secs() >= 30,
            _ => false,
        };
        if to_reason {
            if let Some(FrictionFlow::Countdown { action, .. }) = self.friction_flow.take() {
                self.friction_flow = Some(FrictionFlow::ReasonPrompt {
                    action,
                    value: String::new(),
                });
            }
            return;
        }

        let final_done = match &self.friction_flow {
            Some(FrictionFlow::FinalCountdown { started, .. }) => {
                started.elapsed().as_secs() >= 60
            }
            _ => false,
        };
        if final_done {
            if let Some(FrictionFlow::FinalCountdown { action, reason, .. }) =
                self.friction_flow.take()
            {
                self.pending_action = Some(PendingFriction { action, reason });
            }
        }
    }
}
