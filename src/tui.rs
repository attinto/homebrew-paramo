use crate::blocker::{self, StatusSnapshot};
use crate::config::{SiteMutation, SystemConfig};
use crate::doctor::{self, Diagnostic, DiagnosticLevel};
use crate::i18n::{I18n, Language};
use crate::ipc;
use crate::paths;
use crate::preferences::UserPreferences;
use anyhow::Result;
use chrono::Datelike;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;

const ASCII_ART: &[&str] = &[
    r"                                   /\",
    r"                              /\  //\\",
    r"                       /\    //\\///\\\        /\",
    r"                      //\\  ///\////\\\\  /\  //\\",
    r"         /\          /  ^ \/^ ^/^  ^  ^ \/^ \/  ^ \",
    r"        / ^\    /\  / ^   /  ^/ ^ ^ ^   ^\ ^/  ^^  \",
    r"       /^   \  / ^\/ ^ ^   ^ / ^  ^    ^  \/ ^   ^  \       *",
    r"      /  ^ ^ \/^  ^\ ^ ^ ^   ^  ^   ^   ____  ^   ^  \     /|\",
    r"     / ^ ^  ^ \ ^  _\___________________|  |_____^ ^  \   /||o\",
    r"    / ^^  ^ ^ ^\  /______________________________\ ^ ^ \ /|o|||\",
    r"   /  ^  ^^ ^ ^  /________________________________\  ^  /|||||o|\",
    r"  /^ ^  ^ ^^  ^    ||___|___||||||||||||___|__|||      /||o||||||\",
    r" / ^   ^   ^    ^  ||___|___||||||||||||___|__|||          | |",
    r"/ ^ ^ ^  ^  ^  ^   ||||||||||||||||||||||||||||||oooooooooo| |ooooooo",
    r"ooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo",
];

#[derive(Debug, Clone, Copy)]
enum TabId {
    Home,
    Sites,
    Schedule,
    Settings,
    Diagnostics,
    Exit,
}

impl TabId {
    fn all() -> [Self; 6] {
        [
            Self::Home,
            Self::Sites,
            Self::Schedule,
            Self::Settings,
            Self::Diagnostics,
            Self::Exit,
        ]
    }

    fn index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Sites => 1,
            Self::Schedule => 2,
            Self::Settings => 3,
            Self::Diagnostics => 4,
            Self::Exit => 5,
        }
    }

    fn from_index(index: usize) -> Self {
        Self::all()[index % Self::all().len()]
    }
}

#[derive(Debug, Clone)]
struct PromptState {
    title: String,
    value: String,
}

#[derive(Debug)]
struct Dashboard {
    config: SystemConfig,
    prefs: UserPreferences,
    i18n: I18n,
    status: StatusSnapshot,
    diagnostics: Vec<Diagnostic>,
    active_tab: TabId,
    sites_state: ListState,
    diagnostics_state: ListState,
    schedule_cursor: usize,
    flash_message: Option<String>,
    prompt: Option<PromptState>,
}

pub fn run(config: &mut SystemConfig, prefs: &mut UserPreferences) -> Result<()> {
    let mut dashboard = Dashboard::new(config.clone(), prefs.clone())?;
    let mut terminal = TerminalSession::new()?;

    loop {
        terminal.terminal.draw(|frame| dashboard.render(frame))?;

        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if dashboard.handle_key(key)? {
                    break;
                }
            }
        }
    }

    *config = dashboard.config;
    *prefs = dashboard.prefs;
    Ok(())
}

impl Dashboard {
    fn new(config: SystemConfig, prefs: UserPreferences) -> Result<Self> {
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
        })
    }

    fn render(&mut self, frame: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(18),
                Constraint::Length(3),
                Constraint::Min(12),
                Constraint::Length(3),
            ])
            .split(frame.area());

        self.render_header(frame, outer[0]);
        self.render_tabs(frame, outer[1]);
        self.render_body(frame, outer[2]);
        self.render_footer(frame, outer[3]);

        if let Some(prompt) = &self.prompt {
            self.render_prompt(frame, prompt);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(paths::APP_DISPLAY_NAME);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(74), Constraint::Percentage(26)])
            .split(inner);

        let art = ASCII_ART
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    *line,
                    Style::default()
                        .fg(Color::Rgb(177, 214, 166))
                        .add_modifier(Modifier::BOLD),
                ))
            })
            .collect::<Vec<_>>();

        let subtitle = match self.i18n.language() {
            Language::Es => "Aíslate para construir",
            Language::En => "Isolate to build",
        };

        let side_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                paths::APP_DISPLAY_NAME,
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                subtitle,
                Style::default().fg(Color::Rgb(231, 223, 201)),
            )),
            Line::from(""),
            Line::from(Span::styled(
                self.i18n.header_nav_hint(),
                Style::default().fg(Color::Rgb(190, 197, 208)),
            )),
            Line::from(Span::styled(
                self.i18n.header_confirm_hint(),
                Style::default().fg(Color::Rgb(190, 197, 208)),
            )),
            Line::from(Span::styled(
                self.i18n.header_quit_hint(),
                Style::default().fg(Color::Rgb(190, 197, 208)),
            )),
        ];

        frame.render_widget(
            Paragraph::new(Text::from(art))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false }),
            sections[0],
        );
        frame.render_widget(
            Paragraph::new(side_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            sections[1],
        );
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let titles = vec![
            self.i18n.home_tab(),
            self.i18n.sites_tab(),
            self.i18n.schedule_tab(),
            self.i18n.settings_tab(),
            self.i18n.diagnostics_tab(),
            self.i18n.exit_tab(),
        ]
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();

        let tabs = Tabs::new(titles)
            .select(self.active_tab.index())
            .style(Style::default().fg(Color::Rgb(190, 197, 208)))
            .highlight_style(
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )
            .divider(" ")
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(tabs, area);
    }

    fn render_body(&mut self, frame: &mut Frame, area: Rect) {
        match self.active_tab {
            TabId::Home => self.render_home(frame, area),
            TabId::Sites => self.render_sites(frame, area),
            TabId::Schedule => self.render_schedule(frame, area),
            TabId::Settings => self.render_settings(frame, area),
            TabId::Diagnostics => self.render_diagnostics(frame, area),
            TabId::Exit => self.render_exit(frame, area),
        }
    }

    fn render_home(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        let state_label = if self.status.hosts_blocked {
            self.i18n.blocked_label()
        } else {
            self.i18n.unblocked_label()
        };
        let schedule_label = if self.status.schedule_active {
            self.i18n.schedule_active_label()
        } else {
            self.i18n.schedule_inactive_label()
        };

        let next_change = self
            .status
            .next_transition
            .map(|next| next.format("%H:%M").to_string())
            .unwrap_or_else(|| "--:--".to_string());

        let home_lines = vec![
            Line::from(format!(
                "{} {}",
                self.i18n.weekday(self.status.now.weekday()),
                self.status.now.format("%H:%M")
            )),
            Line::from(""),
            Line::from(Span::styled(
                state_label,
                Style::default()
                    .fg(if self.status.hosts_blocked {
                        Color::Rgb(224, 102, 102)
                    } else {
                        Color::Rgb(123, 201, 111)
                    })
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(schedule_label),
            Line::from(format!(
                "{}: {}",
                self.i18n.configured_sites_label(),
                self.status.site_count
            )),
            Line::from(format!(
                "{}: {}",
                self.i18n.next_change_label(),
                next_change
            )),
        ];

        let status_panel = Paragraph::new(home_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(match self.i18n.language() {
                        Language::Es => "Estado",
                        Language::En => "Status",
                    }),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(status_panel, columns[0]);

        let actions = vec![
            Line::from(self.i18n.home_action_block()),
            Line::from(self.i18n.home_action_unblock()),
            Line::from(self.i18n.home_action_refresh()),
            Line::from(self.i18n.home_action_exit()),
        ];

        let actions_panel = Paragraph::new(actions)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.home_actions()),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(actions_panel, columns[1]);
    }

    fn render_sites(&mut self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        let items = if self.config.sites.list.is_empty() {
            vec![ListItem::new(self.i18n.site_empty())]
        } else {
            self.config
                .sites
                .list
                .iter()
                .map(|site| ListItem::new(site.clone()))
                .collect()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.sites_tab()),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("› ");

        frame.render_stateful_widget(list, columns[0], &mut self.sites_state);

        let selected_site = self
            .sites_state
            .selected()
            .and_then(|index| self.config.sites.list.get(index))
            .cloned()
            .unwrap_or_else(|| "-".to_string());

        let side = Paragraph::new(vec![
            Line::from(format!(
                "{}: {}",
                self.i18n.configured_sites_label(),
                self.config.sites.list.len()
            )),
            Line::from(format!(
                "{}: {}",
                match self.i18n.language() {
                    Language::Es => "Seleccionado",
                    Language::En => "Selected",
                },
                selected_site
            )),
            Line::from(""),
            Line::from(match self.i18n.language() {
                Language::Es => "a  Añadir sitio",
                Language::En => "a  Add site",
            }),
            Line::from(match self.i18n.language() {
                Language::Es => "d  Eliminar sitio",
                Language::En => "d  Remove site",
            }),
            Line::from(match self.i18n.language() {
                Language::Es => "↑ ↓  Mover selección",
                Language::En => "↑ ↓  Move selection",
            }),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(match self.i18n.language() {
                    Language::Es => "Gestión",
                    Language::En => "Manage",
                }),
        )
        .wrap(Wrap { trim: true });

        frame.render_widget(side, columns[1]);
    }

    fn render_schedule(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        let rows = [
            format!(
                "{}: {:02}:00",
                self.i18n.start_label(),
                self.config.schedule.start
            ),
            format!(
                "{}: {:02}:00",
                self.i18n.end_label(),
                self.config.schedule.end
            ),
            format!(
                "{}: {}",
                self.i18n.weekends_label(),
                match self.i18n.language() {
                    Language::Es => {
                        if self.config.schedule.block_weekends {
                            "activados"
                        } else {
                            "desactivados"
                        }
                    }
                    Language::En => {
                        if self.config.schedule.block_weekends {
                            "on"
                        } else {
                            "off"
                        }
                    }
                }
            ),
        ];

        let items = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let style = if index == self.schedule_cursor {
                    Style::default()
                        .fg(Color::Rgb(241, 203, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(row.clone(), style)))
            })
            .collect::<Vec<_>>();

        frame.render_widget(
            List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.schedule_tab()),
            ),
            columns[0],
        );

        let side = Paragraph::new(vec![
            Line::from(self.i18n.schedule_summary(
                self.config.schedule.start,
                self.config.schedule.end,
                self.config.schedule.block_weekends,
            )),
            Line::from(""),
            Line::from(self.i18n.schedule_controls()),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(match self.i18n.language() {
                    Language::Es => "Control",
                    Language::En => "Controls",
                }),
        )
        .wrap(Wrap { trim: true });

        frame.render_widget(side, columns[1]);
    }

    fn render_settings(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(format!(
                "{}: {} ({})",
                self.i18n.language_label(),
                self.prefs.language.native_name(),
                self.prefs.language.code()
            )),
            Line::from(match self.i18n.language() {
                Language::Es => "← → cambian el idioma",
                Language::En => "← → changes the language",
            }),
            Line::from(""),
            Line::from(self.i18n.install_note()),
        ];

        frame.render_widget(
            Paragraph::new(content)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(self.i18n.settings_tab()),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_diagnostics(&mut self, frame: &mut Frame, area: Rect) {
        let items = self
            .diagnostics
            .iter()
            .map(|item| {
                let (label, color) = match item.level {
                    DiagnosticLevel::Ok => (self.i18n.ok(), Color::Rgb(123, 201, 111)),
                    DiagnosticLevel::Warn => (self.i18n.warning(), Color::Rgb(241, 203, 126)),
                    DiagnosticLevel::Error => (self.i18n.error(), Color::Rgb(224, 102, 102)),
                };

                let mut lines = vec![
                    Line::from(Span::styled(
                        format!("[{}] {}", label, item.title),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(item.detail.clone()),
                ];

                if let Some(hint) = &item.hint {
                    lines.push(Line::from(hint.clone()));
                }

                ListItem::new(lines)
            })
            .collect::<Vec<_>>();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.doctor_title()),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("• ");

        frame.render_stateful_widget(list, area, &mut self.diagnostics_state);
    }

    fn render_exit(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(Span::styled(
                self.i18n.exit_screen_title(),
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(self.i18n.exit_screen_body()),
            Line::from(self.i18n.exit_screen_hint()),
        ];

        frame.render_widget(
            Paragraph::new(content)
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(self.i18n.exit_tab()),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let fallback = match self.active_tab {
            TabId::Diagnostics => self.i18n.diagnostics_refresh().to_string(),
            TabId::Exit => self.i18n.exit_screen_body().to_string(),
            _ => self.i18n.tui_hint().to_string(),
        };
        let message = self.flash_message.clone().unwrap_or(fallback);

        let paragraph = Paragraph::new(message)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    fn render_prompt(&self, frame: &mut Frame, prompt: &PromptState) {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(prompt.title.clone()),
                Line::from(""),
                Line::from(Span::styled(
                    prompt.value.clone(),
                    Style::default()
                        .fg(Color::Rgb(241, 203, 126))
                        .add_modifier(Modifier::BOLD),
                )),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(match self.i18n.language() {
                        Language::Es => "Entrada",
                        Language::En => "Input",
                    }),
            )
            .alignment(Alignment::Left),
            area,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
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

    fn handle_tab_key(&mut self, key: KeyEvent) -> Result<bool> {
        match self.active_tab {
            TabId::Home => {}
            TabId::Sites => self.handle_sites_key(key)?,
            TabId::Schedule => self.handle_schedule_key(key)?,
            TabId::Settings => self.handle_settings_key(key)?,
            TabId::Diagnostics => self.handle_diagnostics_key(key)?,
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

    fn try_block(&mut self) -> Result<()> {
        match ipc::send_command("block") {
            Ok(r) if r == "ok" => self.set_flash(self.i18n.blocked_now()),
            Ok(err) => self.set_flash(err),
            Err(e) => self.set_flash(e),
        }
        self.refresh_status()?;
        Ok(())
    }

    fn try_unblock(&mut self) -> Result<()> {
        match ipc::send_command("unblock") {
            Ok(r) if r == "ok" => self.set_flash(self.i18n.unblocked_now()),
            Ok(err) => self.set_flash(err),
            Err(e) => self.set_flash(e),
        }
        self.refresh_status()?;
        Ok(())
    }

    fn add_site(&mut self, raw: &str) -> Result<()> {
        match self.config.add_site(raw) {
            Ok(SiteMutation::Added(site)) => {
                self.config.save_active()?;
                let _ = ipc::send_command("sync");
                self.set_flash(self.i18n.site_added(&site));
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
                let _ = ipc::send_command("sync");
                self.set_flash(self.i18n.site_removed(&site));
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
        let _ = ipc::send_command("sync");
        self.set_flash(self.i18n.schedule_updated(start, end, weekends));
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
        self.i18n = I18n::new(self.prefs.language);
        self.set_flash(self.i18n.language_updated(self.prefs.language));
        self.refresh_diagnostics()?;
        Ok(())
    }

    fn refresh_status(&mut self) -> Result<()> {
        self.status = blocker::status_snapshot(&self.config)?;
        Ok(())
    }

    fn refresh_diagnostics(&mut self) -> Result<()> {
        self.diagnostics = doctor::run(&self.config, self.i18n)?;
        if self.diagnostics.is_empty() {
            self.diagnostics_state.select(None);
        } else if self.diagnostics_state.selected().is_none() {
            self.diagnostics_state.select(Some(0));
        }
        Ok(())
    }

    fn set_flash(&mut self, message: impl Into<String>) {
        self.flash_message = Some(message.into());
    }

    fn next_tab(&mut self) {
        self.active_tab = TabId::from_index(self.active_tab.index() + 1);
    }

    fn previous_tab(&mut self) {
        let count = TabId::all().len();
        self.active_tab = TabId::from_index((self.active_tab.index() + count - 1) % count);
    }

    fn select_next_site(&mut self) {
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

    fn select_previous_site(&mut self) {
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

    fn select_next_diagnostic(&mut self) {
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

    fn select_previous_diagnostic(&mut self) {
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
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn wrap_hour(value: u8, delta: i8) -> u8 {
    let raw = value as i16 + delta as i16;
    if raw < 0 {
        23
    } else if raw > 23 {
        0
    } else {
        raw as u8
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
