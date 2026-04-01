use super::helpers::wrap_hour;
use super::state::{Dashboard, PromptState, TabId, UnblockFlow};
use crate::config::SiteMutation;
use crate::i18n::Language;
use crate::ipc;
use crate::journal;
use crate::streak;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

impl Dashboard {
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.unblock_flow.is_some() {
            return self.handle_unblock_flow_key(key);
        }

        if self.prompt.is_some() {
            return self.handle_prompt_key(key);
        }

        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.previous_tab(),
            KeyCode::Char('b') => self.try_block()?,
            KeyCode::Char('u') => self.try_unblock()?,
            KeyCode::Char('r') => self.refresh_status()?,
            _ => return self.handle_tab_key(key),
        }

        Ok(false)
    }

    pub(crate) fn perform_pending_actions(&mut self) -> Result<()> {
        if let Some(reason) = self.pending_unblock.take() {
            let _ = journal::append(&reason);
            self.streak = streak::load().unwrap_or_default();
            self.wall_entries = journal::load().unwrap_or_default();
            match ipc::send_command("unblock") {
                Ok(()) => self.set_flash(self.i18n.unblocked_now()),
                Err(error) => self.set_flash(error),
            }
            self.refresh_status()?;
        }
        Ok(())
    }

    fn handle_tab_key(&mut self, key: KeyEvent) -> Result<bool> {
        match self.active_tab {
            TabId::Home => {}
            TabId::Sites => self.handle_sites_key(key)?,
            TabId::Schedule => self.handle_schedule_key(key)?,
            TabId::Settings => self.handle_settings_key(key)?,
            TabId::Diagnostics => self.handle_diagnostics_key(key)?,
            TabId::Streak => {}
            TabId::Wall => self.handle_wall_key(key),
            TabId::Exit => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn handle_sites_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Down => self.select_next_site(),
            KeyCode::Up => self.select_previous_site(),
            KeyCode::Char('a') => {
                self.prompt = Some(PromptState {
                    title: self.i18n.add_site_prompt().to_string(),
                    value: String::new(),
                });
            }
            KeyCode::Char('d') => self.remove_selected_site()?,
            _ => {}
        }

        Ok(())
    }

    fn handle_schedule_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Down => self.schedule_cursor = (self.schedule_cursor + 1) % 3,
            KeyCode::Up => self.schedule_cursor = (self.schedule_cursor + 2) % 3,
            KeyCode::Left => self.adjust_schedule(-1)?,
            KeyCode::Right => self.adjust_schedule(1)?,
            _ => {}
        }

        Ok(())
    }

    fn handle_settings_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Enter => self.toggle_language()?,
            _ => {}
        }

        Ok(())
    }

    fn handle_diagnostics_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('g') => self.refresh_diagnostics()?,
            KeyCode::Down => self.select_next_diagnostic(),
            KeyCode::Up => self.select_previous_diagnostic(),
            _ => {}
        }

        Ok(())
    }

    fn handle_wall_key(&mut self, key: KeyEvent) {
        if self.wall_entries.is_empty() {
            return;
        }
        let len = self.wall_entries.len();
        match key.code {
            KeyCode::Down => {
                let next = match self.wall_state.selected() {
                    Some(i) => (i + 1) % len,
                    None => 0,
                };
                self.wall_state.select(Some(next));
            }
            KeyCode::Up => {
                let next = match self.wall_state.selected() {
                    Some(i) => (i + len - 1) % len,
                    None => 0,
                };
                self.wall_state.select(Some(next));
            }
            _ => {}
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> Result<bool> {
        let prompt = self.prompt.as_mut().expect("prompt must exist");
        match key.code {
            KeyCode::Esc => {
                self.prompt = None;
            }
            KeyCode::Enter => {
                let value = prompt.value.clone();
                self.prompt = None;
                self.add_site(value.trim())?;
            }
            KeyCode::Backspace => {
                prompt.value.pop();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                prompt.value.push(ch);
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_unblock_flow_key(&mut self, key: KeyEvent) -> Result<bool> {
        if matches!(key.code, KeyCode::Esc) {
            self.unblock_flow = None;
            self.set_flash(self.i18n.unblock_cancelled());
            return Ok(false);
        }

        let is_reason_prompt = matches!(self.unblock_flow, Some(UnblockFlow::ReasonPrompt { .. }));

        if is_reason_prompt {
            match key.code {
                KeyCode::Enter => {
                    let reason = match &self.unblock_flow {
                        Some(UnblockFlow::ReasonPrompt { value }) => value.trim().to_string(),
                        _ => String::new(),
                    };
                    if reason.is_empty() {
                        self.set_flash(self.i18n.reason_required());
                    } else {
                        self.unblock_flow = Some(UnblockFlow::FinalCountdown {
                            started: Instant::now(),
                            reason,
                        });
                    }
                }
                KeyCode::Backspace => {
                    if let Some(UnblockFlow::ReasonPrompt { value }) = &mut self.unblock_flow {
                        value.pop();
                    }
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(UnblockFlow::ReasonPrompt { value }) = &mut self.unblock_flow {
                        value.push(ch);
                    }
                }
                _ => {}
            }
        }

        Ok(false)
    }

    fn try_block(&mut self) -> Result<()> {
        match ipc::send_command("block") {
            Ok(()) => self.set_flash(self.i18n.blocked_now()),
            Err(error) => self.set_flash(error),
        }
        self.refresh_status()?;
        Ok(())
    }

    fn try_unblock(&mut self) -> Result<()> {
        if self.status.schedule_active {
            self.unblock_flow = Some(UnblockFlow::Countdown {
                started: Instant::now(),
            });
        } else {
            match ipc::send_command("unblock") {
                Ok(()) => self.set_flash(self.i18n.unblocked_now()),
                Err(error) => self.set_flash(error),
            }
            self.refresh_status()?;
        }
        Ok(())
    }

    fn add_site(&mut self, raw: &str) -> Result<()> {
        match self.config.add_site(raw) {
            Ok(SiteMutation::Added(site)) => {
                self.config.save_active()?;
                match ipc::send_command("sync") {
                    Ok(()) => self.set_flash(self.i18n.site_added(&site)),
                    Err(error) => self.set_flash(error),
                }
                self.refresh_status()?;
                self.refresh_diagnostics()?;
                self.sites_state.select(
                    self.config
                        .sites
                        .list
                        .iter()
                        .position(|entry| entry == &site),
                );
            }
            Ok(SiteMutation::AlreadyPresent(site)) => {
                self.set_flash(self.i18n.site_already_present(&site))
            }
            Ok(_) => {}
            Err(error) => self.set_flash(error),
        }

        Ok(())
    }

    fn remove_selected_site(&mut self) -> Result<()> {
        let selected = self
            .sites_state
            .selected()
            .and_then(|index| self.config.sites.list.get(index))
            .cloned();

        let Some(site) = selected else {
            self.set_flash(self.i18n.site_empty());
            return Ok(());
        };

        match self.config.remove_site(&site) {
            Ok(SiteMutation::Removed(site)) => {
                self.config.save_active()?;
                match ipc::send_command("sync") {
                    Ok(()) => self.set_flash(self.i18n.site_removed(&site)),
                    Err(error) => self.set_flash(error),
                }
                self.refresh_status()?;
                self.refresh_diagnostics()?;
                if self.config.sites.list.is_empty() {
                    self.sites_state.select(None);
                } else if let Some(index) = self.sites_state.selected() {
                    let next = index.min(self.config.sites.list.len() - 1);
                    self.sites_state.select(Some(next));
                }
            }
            Ok(SiteMutation::NotFound(site)) => self.set_flash(self.i18n.site_not_found(&site)),
            Ok(_) => {}
            Err(error) => self.set_flash(error),
        }

        Ok(())
    }

    fn adjust_schedule(&mut self, delta: i8) -> Result<()> {
        let mut start = self.config.schedule.start;
        let mut end = self.config.schedule.end;
        let mut weekends = self.config.schedule.block_weekends;

        match self.schedule_cursor {
            0 => start = wrap_hour(start, delta),
            1 => end = wrap_hour(end, delta),
            2 => weekends = !weekends,
            _ => {}
        }

        if let Err(error) = self.config.set_schedule(start, end, weekends) {
            self.set_flash(error);
            return Ok(());
        }

        self.config.save_active()?;
        match ipc::send_command("sync") {
            Ok(()) => self.set_flash(self.i18n.schedule_updated(start, end, weekends)),
            Err(error) => self.set_flash(error),
        }
        self.refresh_status()?;
        self.refresh_diagnostics()?;

        Ok(())
    }

    fn toggle_language(&mut self) -> Result<()> {
        self.prefs.language = match self.prefs.language {
            Language::Es => Language::En,
            Language::En => Language::Es,
        };
        self.prefs.save()?;
        self.i18n = crate::i18n::I18n::new(self.prefs.language);
        self.set_flash(self.i18n.language_updated(self.prefs.language));
        self.refresh_diagnostics()?;
        Ok(())
    }
}
